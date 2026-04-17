use crate::msg::TranslatorStatus;
use crate::transcribe::TranscriptLine;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Gauge, List, ListItem, Paragraph};

pub struct UiState<'a> {
    pub lines: &'a [TranscriptLine],
    pub level: f32,
    pub language: &'a str,
    pub recording: bool,
    pub input_name: &'a str,
    pub model_name: &'a str,
    pub saved_note: Option<&'a str>,
    pub translator_status: TranslatorStatus,
}

pub fn draw(f: &mut Frame, state: &UiState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(1),
        ])
        .split(f.area());

    let level_pct = ((state.level * 400.0).clamp(0.0, 100.0)) as u16;
    let status_label = if state.recording { "REC" } else { "PAUSED" };
    let tr_label = match state.translator_status {
        TranslatorStatus::Loading => "tr=loading",
        TranslatorStatus::Ready => "tr=ready",
        TranslatorStatus::Failed => "tr=off",
    };
    let title = format!(
        " livemd · {} · lang={} · in={} · model={} · {} ",
        status_label, state.language, state.input_name, state.model_name, tr_label
    );
    let gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title(title))
        .gauge_style(Style::default().fg(if state.recording {
            Color::Green
        } else {
            Color::DarkGray
        }))
        .percent(level_pct);
    f.render_widget(gauge, chunks[0]);

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[1]);

    let height = cols[0].height.saturating_sub(2) as usize;
    let start = state.lines.len().saturating_sub(height.max(1));

    let en_items: Vec<ListItem> = state.lines[start..]
        .iter()
        .map(|l| render_cell(l, "en"))
        .collect();
    let ja_items: Vec<ListItem> = state.lines[start..]
        .iter()
        .map(|l| render_cell(l, "ja"))
        .collect();

    f.render_widget(
        List::new(en_items).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" English "),
        ),
        cols[0],
    );
    f.render_widget(
        List::new(ja_items).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" 日本語 "),
        ),
        cols[1],
    );

    let help = match state.saved_note {
        Some(note) => format!(
            " {} · q quit&save · s save · l lang · space pause ",
            note
        ),
        None => " q quit&save · s save · l cycle lang · space pause ".to_string(),
    };
    let help_p = Paragraph::new(help).style(Style::default().fg(Color::DarkGray));
    f.render_widget(help_p, chunks[2]);
}

fn render_cell<'a>(line: &'a TranscriptLine, col_lang: &str) -> ListItem<'a> {
    let ts = line.started_at.format("%H:%M:%S").to_string();
    let speaker = line
        .speaker
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(|s| format!("{}: ", s))
        .unwrap_or_default();

    let is_source = line.src_lang == col_lang;
    let (text, style) = if is_source {
        (line.text.as_str(), Style::default())
    } else {
        match line.translated.as_deref() {
            Some(t) => (t, Style::default().fg(Color::Gray)),
            None => ("…", Style::default().fg(Color::DarkGray)),
        }
    };

    let marker = if is_source { "▶ " } else { "  " };
    let head = format!("[{}] {}{}", ts, marker, speaker);

    ListItem::new(Line::from(vec![
        Span::styled(head, Style::default().fg(Color::Cyan)),
        Span::styled(text.to_string(), style),
    ]))
}
