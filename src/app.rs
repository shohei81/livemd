use crate::{
    audio::AudioCapture,
    config::Config,
    markdown,
    msg::{TranslatorStatus, UiMsg},
    transcribe::{Segment, TranscribeRunner, TranscriptLine},
    translate::{self, TranslatorConfig},
    ui::{draw, UiState},
    vad::VadRunner,
};
use anyhow::Result;
use crossbeam_channel::{bounded, unbounded, Receiver};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::prelude::*;
use std::io::{self, Stdout};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;
use tracing::info;

pub fn run(cfg: Config, existing_content: Option<String>) -> Result<()> {
    let (audio_tx, audio_rx) = bounded::<Vec<f32>>(64);
    let (seg_tx, seg_rx) = bounded::<Segment>(16);
    let (line_tx, line_rx) = bounded::<TranscriptLine>(32);
    let (ui_tx, ui_rx) = unbounded::<UiMsg>();
    let (level_tx, level_rx) = bounded::<f32>(16);

    let language = Arc::new(RwLock::new(cfg.language.clone()));

    let capture = AudioCapture::start(&cfg.input_device, audio_tx)?;
    let input_name = capture.input_name.clone();
    let model_name = cfg
        .model_path
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "unknown".into());

    let paused = Arc::new(AtomicBool::new(false));
    let paused_vad = paused.clone();
    let vad_cfg = cfg.vad.clone();
    thread::spawn(move || {
        let vad = VadRunner::new(
            vad_cfg.aggressiveness,
            vad_cfg.min_speech_ms,
            vad_cfg.silence_ms,
            vad_cfg.max_segment_ms,
        );
        if let Err(e) = vad.run(audio_rx, seg_tx, level_tx, paused_vad) {
            tracing::error!("vad thread ended: {}", e);
        }
    });

    let model_path = cfg.model_path.clone();
    let threads_n = cfg.threads;
    let lang_for_worker = language.clone();
    thread::spawn(move || match TranscribeRunner::new(&model_path, threads_n, lang_for_worker) {
        Ok(runner) => runner.run(seg_rx, line_tx),
        Err(e) => tracing::error!("whisper init failed: {}", e),
    });

    // Translator fanout: mirror transcript lines to UI immediately so the source
    // text always renders even if translation is slow or disabled.
    let translator_cfg = cfg
        .translator
        .clone()
        .filter(|t| {
            if !t.model_path.exists() {
                tracing::warn!(
                    path = %t.model_path.display(),
                    "translator model not found — running without translation"
                );
                return false;
            }
            if let Ok(meta) = std::fs::metadata(&t.model_path) {
                if meta.len() < 10_000_000 {
                    tracing::error!(
                        path = %t.model_path.display(),
                        size = meta.len(),
                        "translator model file is too small — likely a broken download"
                    );
                    return false;
                }
            }
            true
        });

    let translator_status_init = match translator_cfg {
        Some(tcfg) => {
            let (trans_in_tx, trans_in_rx) = bounded::<TranscriptLine>(32);
            let ui_tx_fanout = ui_tx.clone();
            thread::spawn(move || {
                while let Ok(line) = line_rx.recv() {
                    let _ = ui_tx_fanout.send(UiMsg::NewLine(line.clone()));
                    let _ = trans_in_tx.send(line);
                }
            });
            translate::spawn(
                TranslatorConfig {
                    binary: tcfg.binary,
                    model_path: tcfg.model_path,
                    port: tcfg.port,
                    n_ctx: tcfg.n_ctx,
                    max_new_tokens: tcfg.max_new_tokens,
                    startup_timeout_secs: tcfg.startup_timeout_secs,
                    log_dir: cfg.log_dir.clone(),
                },
                trans_in_rx,
                ui_tx.clone(),
            );
            TranslatorStatus::Loading
        }
        None => {
            let ui_tx_fwd = ui_tx.clone();
            thread::spawn(move || {
                while let Ok(line) = line_rx.recv() {
                    let _ = ui_tx_fwd.send(UiMsg::NewLine(line));
                }
            });
            TranslatorStatus::Failed
        }
    };

    let mut terminal = setup_terminal()?;
    let res = run_loop(
        &mut terminal,
        &cfg,
        &language,
        &paused,
        &input_name,
        &model_name,
        ui_rx,
        level_rx,
        translator_status_init,
        existing_content.as_deref(),
    );
    restore_terminal(&mut terminal)?;
    drop(capture);
    res
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    cfg: &Config,
    language: &Arc<RwLock<String>>,
    paused: &Arc<AtomicBool>,
    input_name: &str,
    model_name: &str,
    ui_rx: Receiver<UiMsg>,
    level_rx: Receiver<f32>,
    initial_translator_status: TranslatorStatus,
    existing_content: Option<&str>,
) -> Result<()> {
    let mut lines: Vec<TranscriptLine> = Vec::new();
    let mut level = 0.0f32;
    let mut level_smooth = 0.0f32;
    let mut saved_note: Option<String> = None;
    let mut translator_status = initial_translator_status;

    loop {
        while let Ok(msg) = ui_rx.try_recv() {
            match msg {
                UiMsg::NewLine(line) => lines.push(line),
                UiMsg::TranslationReady { id, translated } => {
                    if let Some(line) = lines.iter_mut().find(|l| l.id == id) {
                        line.translated = Some(translated);
                    }
                }
                UiMsg::TranslatorStatus(s) => translator_status = s,
            }
        }
        while let Ok(l) = level_rx.try_recv() {
            level = l;
        }
        level_smooth = level_smooth * 0.7 + level * 0.3;

        let lang = language.read().map(|g| g.clone()).unwrap_or_default();
        let is_recording = !paused.load(Ordering::Relaxed);
        terminal.draw(|f| {
            draw(
                f,
                &UiState {
                    lines: &lines,
                    level: level_smooth,
                    language: &lang,
                    recording: is_recording,
                    input_name,
                    model_name,
                    saved_note: saved_note.as_deref(),
                    translator_status,
                },
            );
        })?;

        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(KeyEvent {
                code, modifiers, ..
            }) = event::read()?
            {
                match (code, modifiers) {
                    (KeyCode::Char('q'), _)
                    | (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                        markdown::write(&cfg.output_path, &lines, existing_content)?;
                        info!(path = %cfg.output_path.display(), "saved on quit");
                        break;
                    }
                    (KeyCode::Char('s'), _) => {
                        markdown::write(&cfg.output_path, &lines, existing_content)?;
                        saved_note = Some(format!("saved → {}", cfg.output_path.display()));
                    }
                    (KeyCode::Char('l'), _) => {
                        if let Ok(mut g) = language.write() {
                            *g = match g.as_str() {
                                "en" => "ja".into(),
                                "ja" => "auto".into(),
                                _ => "en".into(),
                            };
                        }
                    }
                    (KeyCode::Char(' '), _) => {
                        let now_paused = !paused.load(Ordering::Relaxed);
                        paused.store(now_paused, Ordering::Relaxed);
                        info!(paused = now_paused, "toggle recording");
                    }
                    _ => {}
                }
            }
        }
    }
    Ok(())
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    crossterm::execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    Ok(Terminal::new(backend)?)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    crossterm::execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}
