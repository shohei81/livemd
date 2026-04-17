use crate::msg::{DiarizerStatus, UiMsg};
use crate::transcribe::Segment;
use anyhow::{anyhow, Context, Result};
use crossbeam_channel::{Receiver, Sender};
use sherpa_rs::speaker_id::{EmbeddingExtractor, ExtractorConfig};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

#[derive(Clone)]
pub struct DiarizerConfig {
    pub model_path: PathBuf,
    pub threshold: f32,
    pub num_threads: Option<usize>,
    pub min_samples: usize,
}

pub fn spawn(cfg: DiarizerConfig, seg_rx: Receiver<Arc<Segment>>, ui_tx: Sender<UiMsg>) {
    std::thread::spawn(move || {
        let _ = ui_tx.send(UiMsg::DiarizerStatus(DiarizerStatus::Loading));
        match run(&cfg, &seg_rx, &ui_tx) {
            Ok(()) => info!("diarizer thread exiting cleanly"),
            Err(e) => {
                error!("diarizer thread failed: {e:#}");
                let _ = ui_tx.send(UiMsg::DiarizerStatus(DiarizerStatus::Failed));
                while seg_rx.recv().is_ok() {}
            }
        }
    });
}

fn run(
    cfg: &DiarizerConfig,
    seg_rx: &Receiver<Arc<Segment>>,
    ui_tx: &Sender<UiMsg>,
) -> Result<()> {
    let model_str = cfg
        .model_path
        .to_str()
        .context("diarizer model path not utf-8")?
        .to_string();

    let mut extractor = EmbeddingExtractor::new(ExtractorConfig {
        model: model_str,
        num_threads: cfg.num_threads,
        debug: false,
        provider: None,
    })
    .map_err(|e| {
        anyhow!(
            "loading diarizer model {}: {}",
            cfg.model_path.display(),
            e
        )
    })?;
    info!(
        path = %cfg.model_path.display(),
        dim = extractor.embedding_size,
        "diarizer model loaded"
    );
    let _ = ui_tx.send(UiMsg::DiarizerStatus(DiarizerStatus::Ready));

    let mut centroids: Vec<Vec<f32>> = Vec::new();
    let threshold = cfg.threshold;

    while let Ok(seg) = seg_rx.recv() {
        if seg.samples.len() < cfg.min_samples {
            debug!(id = seg.id, len = seg.samples.len(), "segment too short for diarization");
            continue;
        }
        let pcm = seg.samples.clone();
        let embedding = match extractor.compute_speaker_embedding(pcm, 16_000) {
            Ok(e) => e,
            Err(e) => {
                warn!(id = seg.id, error = %e, "embedding failed");
                continue;
            }
        };

        let idx = assign_speaker(&mut centroids, &embedding, threshold);
        let label = format!("S{}", idx + 1);
        debug!(id = seg.id, label = %label, speakers = centroids.len(), "diarized");
        let _ = ui_tx.send(UiMsg::SpeakerReady {
            id: seg.id,
            speaker: label,
        });
    }
    Ok(())
}

fn assign_speaker(centroids: &mut Vec<Vec<f32>>, embedding: &[f32], threshold: f32) -> usize {
    let best = centroids
        .iter()
        .enumerate()
        .map(|(i, c)| (i, cosine(c, embedding)))
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

    match best {
        Some((i, score)) if score >= threshold => {
            // Exponential moving average toward the new embedding.
            let c = &mut centroids[i];
            for (x, y) in c.iter_mut().zip(embedding.iter()) {
                *x = 0.9 * *x + 0.1 * *y;
            }
            i
        }
        _ => {
            centroids.push(embedding.to_vec());
            centroids.len() - 1
        }
    }
}

fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let (mut dot, mut na, mut nb) = (0.0f32, 0.0f32, 0.0f32);
    for i in 0..a.len() {
        dot += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    let denom = na.sqrt() * nb.sqrt() + 1e-9;
    dot / denom
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosine_similar_vectors() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine(&a, &b) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_orthogonal() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        assert!(cosine(&a, &b).abs() < 1e-6);
    }

    #[test]
    fn assign_creates_new_speaker_below_threshold() {
        let mut centroids: Vec<Vec<f32>> = vec![vec![1.0, 0.0, 0.0]];
        let new = vec![0.0, 1.0, 0.0]; // orthogonal
        let idx = assign_speaker(&mut centroids, &new, 0.5);
        assert_eq!(idx, 1);
        assert_eq!(centroids.len(), 2);
    }

    #[test]
    fn assign_reuses_matching_speaker() {
        let mut centroids: Vec<Vec<f32>> = vec![vec![1.0, 0.0, 0.0]];
        let similar = vec![0.9, 0.1, 0.0];
        let idx = assign_speaker(&mut centroids, &similar, 0.5);
        assert_eq!(idx, 0);
        assert_eq!(centroids.len(), 1);
    }
}
