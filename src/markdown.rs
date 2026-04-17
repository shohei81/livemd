use crate::transcribe::TranscriptLine;
use anyhow::Result;
use chrono::Local;
use std::io::Write;
use std::path::Path;

pub fn write(path: &Path, lines: &[TranscriptLine]) -> Result<()> {
    let mut f = std::fs::File::create(path)?;
    let started = lines
        .first()
        .map(|l| l.started_at)
        .unwrap_or_else(Local::now);
    let ended = lines
        .last()
        .map(|l| l.ended_at)
        .unwrap_or_else(Local::now);

    writeln!(f, "---")?;
    writeln!(f, "title: Transcript")?;
    writeln!(f, "started: {}", started.format("%Y-%m-%d %H:%M:%S"))?;
    writeln!(f, "ended: {}", ended.format("%Y-%m-%d %H:%M:%S"))?;
    writeln!(f, "---")?;
    writeln!(f)?;
    writeln!(f, "| time | speaker | English | 日本語 |")?;
    writeln!(f, "|------|---------|---------|--------|")?;

    for line in lines {
        let ts = line.started_at.format("%H:%M:%S");
        let speaker = line.speaker.as_deref().unwrap_or("");
        let (en, ja) = if line.src_lang == "ja" {
            (line.translated.as_deref().unwrap_or(""), line.text.as_str())
        } else {
            (line.text.as_str(), line.translated.as_deref().unwrap_or(""))
        };
        writeln!(
            f,
            "| {} | {} | {} | {} |",
            ts,
            escape_cell(speaker),
            escape_cell(en),
            escape_cell(ja)
        )?;
    }
    Ok(())
}

fn escape_cell(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}
