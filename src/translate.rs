use crate::msg::{TranslatorStatus, UiMsg};
use crate::transcribe::TranscriptLine;
use anyhow::{anyhow, Context, Result};
use crossbeam_channel::{Receiver, Sender};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};
use tracing::{debug, error, info, warn};

const CONTEXT_PAIRS: usize = 3;

struct ContextWindow {
    pairs: VecDeque<(String, String, String)>, // (src_lang, src_text, dst_text)
}

impl ContextWindow {
    fn new() -> Self {
        Self {
            pairs: VecDeque::with_capacity(CONTEXT_PAIRS + 1),
        }
    }

    fn push(&mut self, src_lang: &str, src: &str, dst: &str) {
        self.pairs
            .push_back((src_lang.to_string(), src.to_string(), dst.to_string()));
        while self.pairs.len() > CONTEXT_PAIRS {
            self.pairs.pop_front();
        }
    }

    fn render(&self) -> String {
        if self.pairs.is_empty() {
            return String::new();
        }
        let mut s = String::from("\n\nRecent context (for reference, do NOT re-translate):\n");
        for (lang, src, dst) in &self.pairs {
            let arrow = if lang == "ja" { "JA→EN" } else { "EN→JA" };
            s.push_str(&format!("- [{arrow}] {src} → {dst}\n"));
        }
        s
    }
}

#[derive(Clone)]
pub struct TranslatorConfig {
    pub binary: PathBuf,
    pub model_path: PathBuf,
    pub port: u16,
    pub n_ctx: u32,
    pub max_new_tokens: u32,
    pub startup_timeout_secs: u64,
    pub log_dir: PathBuf,
}

struct ServerHandle {
    child: Option<Child>,
}

impl ServerHandle {
    fn start(cfg: &TranslatorConfig) -> Result<Self> {
        std::fs::create_dir_all(&cfg.log_dir).ok();
        let log_path = cfg.log_dir.join("llama-server.log");
        let log_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .with_context(|| format!("opening {}", log_path.display()))?;
        let log_err = log_file.try_clone()?;

        let child = Command::new(&cfg.binary)
            .arg("--model")
            .arg(&cfg.model_path)
            .arg("--port")
            .arg(cfg.port.to_string())
            .arg("--host")
            .arg("127.0.0.1")
            .arg("--ctx-size")
            .arg(cfg.n_ctx.to_string())
            .arg("--n-gpu-layers")
            .arg("999")
            .arg("--threads")
            .arg("4")
            .stdin(Stdio::null())
            .stdout(Stdio::from(log_file))
            .stderr(Stdio::from(log_err))
            .spawn()
            .with_context(|| {
                format!(
                    "spawning llama-server ({}). Install it with `brew install llama.cpp` or set translator.binary in livemd.toml.",
                    cfg.binary.display()
                )
            })?;
        info!(pid = child.id(), port = cfg.port, "llama-server spawned");
        Ok(Self { child: Some(child) })
    }
}

impl Drop for ServerHandle {
    fn drop(&mut self) {
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            let _ = c.wait();
            info!("llama-server stopped");
        }
    }
}

pub fn spawn(cfg: TranslatorConfig, line_rx: Receiver<TranscriptLine>, ui_tx: Sender<UiMsg>) {
    std::thread::spawn(move || {
        let _ = ui_tx.send(UiMsg::TranslatorStatus(TranslatorStatus::Loading));
        match run(&cfg, &line_rx, &ui_tx) {
            Ok(()) => info!("translator thread exiting cleanly"),
            Err(e) => {
                error!("translator thread failed: {e:#}");
                let _ = ui_tx.send(UiMsg::TranslatorStatus(TranslatorStatus::Failed));
                // Drain and drop — the fanout thread already sent NewLine to the UI.
                while line_rx.recv().is_ok() {}
            }
        }
    });
}

fn run(
    cfg: &TranslatorConfig,
    line_rx: &Receiver<TranscriptLine>,
    ui_tx: &Sender<UiMsg>,
) -> Result<()> {
    let server = ServerHandle::start(cfg)?;

    wait_for_health(cfg).context("waiting for llama-server readiness")?;
    let _ = ui_tx.send(UiMsg::TranslatorStatus(TranslatorStatus::Ready));
    info!("llama-server ready");

    let mut context = ContextWindow::new();

    while let Ok(line) = line_rx.recv() {
        let id = line.id;
        match translate_once(cfg, &line, &context) {
            Ok(translated) if !translated.is_empty() => {
                debug!(id, text = %translated, "translation complete");
                context.push(&line.src_lang, &line.text, &translated);
                let _ = ui_tx.send(UiMsg::TranslationReady { id, translated });
            }
            Ok(_) => {
                debug!(id, "empty translation");
            }
            Err(e) => {
                warn!(id, error = %e, "translation failed");
            }
        }
    }

    drop(server);
    Ok(())
}

fn wait_for_health(cfg: &TranslatorConfig) -> Result<()> {
    let url = format!("http://127.0.0.1:{}/health", cfg.port);
    let deadline = Instant::now() + Duration::from_secs(cfg.startup_timeout_secs);
    let mut last_err: Option<String> = None;
    while Instant::now() < deadline {
        match ureq::get(&url).timeout(Duration::from_secs(2)).call() {
            Ok(resp) if resp.status() == 200 => return Ok(()),
            Ok(resp) => last_err = Some(format!("status {}", resp.status())),
            Err(e) => last_err = Some(e.to_string()),
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    Err(anyhow!(
        "llama-server did not become ready within {}s (last: {})",
        cfg.startup_timeout_secs,
        last_err.unwrap_or_else(|| "unknown".into())
    ))
}

fn translate_once(
    cfg: &TranslatorConfig,
    line: &TranscriptLine,
    context: &ContextWindow,
) -> Result<String> {
    let (src, dst) = if line.src_lang == "ja" {
        ("Japanese", "English")
    } else {
        ("English", "Japanese")
    };

    let ctx_block = context.render();
    let system = format!(
        "You are a professional simultaneous interpreter translating from {src} to {dst}.\n\
Rules:\n\
- Output ONLY the translation. No explanations, no quotes, no prefixes.\n\
- If the input is a sentence fragment (no period, cut mid-thought), translate it as a fragment. Do NOT pad or complete.\n\
- Preserve tone: formal stays formal, casual stays casual.\n\
- Keep proper nouns, technical terms, and numbers exact.\n\
- Match punctuation style of the target language.{ctx_block}"
    );
    let prompt = format!(
        "<|im_start|>system\n{system}<|im_end|>\n<|im_start|>user\n{}<|im_end|>\n<|im_start|>assistant\n",
        line.text
    );

    let url = format!("http://127.0.0.1:{}/completion", cfg.port);
    let body = serde_json::json!({
        "prompt": prompt,
        "n_predict": cfg.max_new_tokens,
        "temperature": 0.2,
        "top_p": 0.9,
        "repeat_penalty": 1.1,
        "stop": ["<|im_end|>", "<|endoftext|>"],
        "cache_prompt": true
    });

    let resp: serde_json::Value = ureq::post(&url)
        .timeout(Duration::from_secs(30))
        .send_json(body)?
        .into_json()?;

    let text = resp
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    Ok(text)
}
