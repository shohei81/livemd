use crate::transcribe::TranscriptLine;

#[derive(Clone)]
pub enum UiMsg {
    NewLine(TranscriptLine),
    TranslationReady { id: u64, translated: String },
    TranslatorStatus(TranslatorStatus),
}

#[derive(Clone, Copy, Debug)]
pub enum TranslatorStatus {
    Loading,
    Ready,
    Failed,
}

pub fn detect_lang(text: &str) -> &'static str {
    let has_cjk = text.chars().any(|c| {
        let n = c as u32;
        (0x3040..=0x309F).contains(&n)
            || (0x30A0..=0x30FF).contains(&n)
            || (0x4E00..=0x9FFF).contains(&n)
            || (0xFF66..=0xFF9D).contains(&n)
    });
    if has_cjk {
        "ja"
    } else {
        "en"
    }
}
