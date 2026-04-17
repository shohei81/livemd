use crate::audio::TARGET_SR;
use crate::transcribe::Segment;
use anyhow::Result;
use chrono::{DateTime, Local};
use crossbeam_channel::{Receiver, Sender};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tracing::{debug, info};
use webrtc_vad::{SampleRate as VadSr, Vad, VadMode};

const FRAME_MS: u32 = 30;
pub const FRAME_SAMPLES: usize = (TARGET_SR as usize * FRAME_MS as usize) / 1000; // 480

pub struct VadRunner {
    aggressiveness: u8,
    min_speech_ms: u32,
    silence_ms: u32,
    max_segment_ms: u32,
}

impl VadRunner {
    pub fn new(aggr: u8, min_speech_ms: u32, silence_ms: u32, max_segment_ms: u32) -> Self {
        Self {
            aggressiveness: aggr,
            min_speech_ms,
            silence_ms,
            max_segment_ms,
        }
    }

    pub fn run(
        &self,
        audio_rx: Receiver<Vec<f32>>,
        seg_tx: Sender<Segment>,
        level_tx: Sender<f32>,
        paused: Arc<AtomicBool>,
    ) -> Result<()> {
        let mode = match self.aggressiveness {
            0 => VadMode::Quality,
            1 => VadMode::LowBitrate,
            2 => VadMode::Aggressive,
            _ => VadMode::VeryAggressive,
        };
        let mut vad = Vad::new_with_rate_and_mode(VadSr::Rate16kHz, mode);

        let mut leftover: Vec<f32> = Vec::with_capacity(FRAME_SAMPLES * 4);
        let mut segment: Vec<f32> = Vec::new();
        let mut in_speech = false;
        let mut silence_frames = 0usize;
        let mut speech_frames = 0usize;
        let silence_limit = (self.silence_ms / FRAME_MS).max(1) as usize;
        let min_speech_frames = (self.min_speech_ms / FRAME_MS).max(1) as usize;
        let max_segment_frames = (self.max_segment_ms / FRAME_MS).max(1) as usize;
        let mut segment_start: Option<DateTime<Local>> = None;
        let mut next_id: u64 = 0;

        while let Ok(chunk) = audio_rx.recv() {
            if paused.load(Ordering::Relaxed) {
                // Discard audio, reset segment state, flatten the UI gauge.
                leftover.clear();
                segment.clear();
                in_speech = false;
                speech_frames = 0;
                silence_frames = 0;
                let _ = level_tx.try_send(0.0);
                continue;
            }

            leftover.extend_from_slice(&chunk);

            while leftover.len() >= FRAME_SAMPLES {
                let frame_f: Vec<f32> = leftover.drain(..FRAME_SAMPLES).collect();
                let frame_i16: Vec<i16> = frame_f
                    .iter()
                    .map(|s| (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
                    .collect();

                let rms = (frame_f.iter().map(|s| s * s).sum::<f32>() / frame_f.len() as f32).sqrt();
                let _ = level_tx.try_send(rms);

                let is_speech = vad.is_voice_segment(&frame_i16).unwrap_or(false);

                if is_speech {
                    if !in_speech {
                        in_speech = true;
                        speech_frames = 0;
                        segment_start = Some(Local::now());
                        debug!("speech start");
                    }
                    silence_frames = 0;
                    speech_frames += 1;
                    segment.extend_from_slice(&frame_f);
                } else if in_speech {
                    silence_frames += 1;
                    segment.extend_from_slice(&frame_f);
                    if silence_frames >= silence_limit {
                        if speech_frames >= min_speech_frames {
                            let speech_ms = (speech_frames as u32) * FRAME_MS;
                            let id = next_id;
                            next_id += 1;
                            let seg = Segment {
                                id,
                                samples: std::mem::take(&mut segment),
                                started_at: segment_start.unwrap_or_else(Local::now),
                                ended_at: Local::now(),
                                speech_ms,
                            };
                            info!(
                                id,
                                speech_frames,
                                speech_ms,
                                len = seg.samples.len(),
                                "flushing segment"
                            );
                            let _ = seg_tx.send(seg);
                        } else {
                            debug!(speech_frames, "discarded short segment");
                            segment.clear();
                        }
                        in_speech = false;
                        silence_frames = 0;
                        speech_frames = 0;
                    }
                }

                if in_speech && (speech_frames + silence_frames) >= max_segment_frames {
                    let speech_ms = (speech_frames as u32) * FRAME_MS;
                    let id = next_id;
                    next_id += 1;
                    let seg = Segment {
                        id,
                        samples: std::mem::take(&mut segment),
                        started_at: segment_start.unwrap_or_else(Local::now),
                        ended_at: Local::now(),
                        speech_ms,
                    };
                    info!(id, "max segment reached, force-flushing");
                    let _ = seg_tx.send(seg);
                    in_speech = false;
                    silence_frames = 0;
                    speech_frames = 0;
                }
            }
        }
        Ok(())
    }
}
