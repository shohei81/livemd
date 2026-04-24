use anyhow::{anyhow, Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream, StreamConfig};
use crossbeam_channel::Sender;
use rubato::{FftFixedInOut, Resampler};
use tracing::{info, warn};

pub const TARGET_SR: u32 = 16_000;

/// Prefix used on loopback (system-audio) sources so we can distinguish them
/// from real input devices that happen to share the same name.
pub const LOOPBACK_PREFIX: &str = "loopback:";

/// Returns selectable audio sources: "default" + input devices + output
/// devices prefixed with [`LOOPBACK_PREFIX`] for system-audio capture.
///
/// On Windows, output devices can be opened as input via WASAPI loopback.
/// On macOS/Linux, loopback entries are enumerated too, but opening them
/// will fail unless a virtual driver (e.g. BlackHole) is in use — in that
/// case the virtual driver already appears as a regular input device.
pub fn list_input_devices() -> Vec<String> {
    let mut out = vec!["default".to_string()];
    let host = cpal::default_host();
    if let Ok(devs) = host.input_devices() {
        for d in devs {
            if let Ok(name) = d.name() {
                if !out.iter().any(|existing| existing == &name) {
                    out.push(name);
                }
            }
        }
    }
    if let Ok(devs) = host.output_devices() {
        for d in devs {
            if let Ok(name) = d.name() {
                let labeled = format!("{}{}", LOOPBACK_PREFIX, name);
                if !out.iter().any(|existing| existing == &labeled) {
                    out.push(labeled);
                }
            }
        }
    }
    out
}

pub struct AudioCapture {
    _stream: Stream,
    pub input_name: String,
    #[allow(dead_code)]
    pub input_sr: u32,
    #[allow(dead_code)]
    pub input_channels: u16,
}

impl AudioCapture {
    pub fn start(device_name: &str, tx: Sender<Vec<f32>>) -> Result<Self> {
        let host = cpal::default_host();
        let (device, is_loopback) = if let Some(name) = device_name.strip_prefix(LOOPBACK_PREFIX) {
            let dev = host
                .output_devices()?
                .find(|d| d.name().map(|n| n == name).unwrap_or(false))
                .ok_or_else(|| anyhow!("output device not found: {}", name))?;
            if !cfg!(target_os = "windows") {
                warn!(
                    device = %name,
                    "loopback capture is only natively supported on Windows \
                     (WASAPI). On macOS, install a virtual audio driver \
                     (e.g. BlackHole) and select it as a regular input device. \
                     On Linux (PulseAudio), select the *.monitor source from \
                     the input list."
                );
            }
            (dev, true)
        } else if device_name == "default" {
            (
                host.default_input_device()
                    .ok_or_else(|| anyhow!("no default input device"))?,
                false,
            )
        } else {
            let dev = host
                .input_devices()?
                .find(|d| d.name().map(|n| n == device_name).unwrap_or(false))
                .ok_or_else(|| anyhow!("input device not found: {}", device_name))?;
            (dev, false)
        };

        let input_name = if is_loopback {
            format!("{}{}", LOOPBACK_PREFIX, device.name().unwrap_or_else(|_| "unknown".into()))
        } else {
            device.name().unwrap_or_else(|_| "unknown".into())
        };
        // On WASAPI loopback we capture the output format; on real input
        // devices we use the input config.
        let supported = if is_loopback {
            device
                .default_output_config()
                .context("default output config (loopback)")?
        } else {
            device
                .default_input_config()
                .context("default input config")?
        };
        let sample_format = supported.sample_format();
        let config: StreamConfig = supported.into();
        let input_sr = config.sample_rate.0;
        let input_channels = config.channels;

        info!(
            %input_name,
            input_sr,
            input_channels,
            ?sample_format,
            is_loopback,
            "opening input stream"
        );

        let err_fn = |e| warn!("audio stream error: {}", e);

        let stream = match sample_format {
            SampleFormat::F32 => {
                let mut state = ProcState::new(input_channels as usize, input_sr, tx.clone())?;
                device.build_input_stream(
                    &config,
                    move |data: &[f32], _| state.push(data),
                    err_fn,
                    None,
                )?
            }
            SampleFormat::I16 => {
                let mut state = ProcState::new(input_channels as usize, input_sr, tx.clone())?;
                device.build_input_stream(
                    &config,
                    move |data: &[i16], _| {
                        let f: Vec<f32> = data.iter().map(|s| *s as f32 / i16::MAX as f32).collect();
                        state.push(&f);
                    },
                    err_fn,
                    None,
                )?
            }
            SampleFormat::U16 => {
                let mut state = ProcState::new(input_channels as usize, input_sr, tx.clone())?;
                device.build_input_stream(
                    &config,
                    move |data: &[u16], _| {
                        let f: Vec<f32> = data
                            .iter()
                            .map(|s| (*s as f32 - 32768.0) / 32768.0)
                            .collect();
                        state.push(&f);
                    },
                    err_fn,
                    None,
                )?
            }
            other => return Err(anyhow!("unsupported sample format: {:?}", other)),
        };

        stream.play()?;
        Ok(Self {
            _stream: stream,
            input_name,
            input_sr,
            input_channels,
        })
    }
}

struct ProcState {
    channels: usize,
    mono_buf: Vec<f32>,
    resampler: Option<FftFixedInOut<f32>>,
    tx: Sender<Vec<f32>>,
}

impl ProcState {
    fn new(channels: usize, input_sr: u32, tx: Sender<Vec<f32>>) -> Result<Self> {
        let resampler = if input_sr != TARGET_SR {
            Some(FftFixedInOut::<f32>::new(
                input_sr as usize,
                TARGET_SR as usize,
                1024,
                1,
            )?)
        } else {
            None
        };
        Ok(Self {
            channels,
            mono_buf: Vec::with_capacity(8192),
            resampler,
            tx,
        })
    }

    fn push(&mut self, data: &[f32]) {
        if self.channels <= 1 {
            self.mono_buf.extend_from_slice(data);
        } else {
            for frame in data.chunks_exact(self.channels) {
                let s: f32 = frame.iter().sum::<f32>() / self.channels as f32;
                self.mono_buf.push(s);
            }
        }

        if let Some(resampler) = self.resampler.as_mut() {
            let in_size = resampler.input_frames_next();
            while self.mono_buf.len() >= in_size {
                let input: Vec<f32> = self.mono_buf.drain(..in_size).collect();
                match resampler.process(&[input], None) {
                    Ok(mut out) => {
                        if let Some(first) = out.pop() {
                            if !first.is_empty() {
                                let _ = self.tx.try_send(first);
                            }
                        }
                    }
                    Err(e) => warn!("resample error: {}", e),
                }
            }
        } else if !self.mono_buf.is_empty() {
            let chunk: Vec<f32> = std::mem::take(&mut self.mono_buf);
            let _ = self.tx.try_send(chunk);
        }
    }
}
