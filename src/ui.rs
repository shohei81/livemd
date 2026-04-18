use crate::app::DevicePicker;
use crate::msg::{DraftState, TranslatorStatus};
use crate::transcribe::TranscriptLine;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Gauge, List, ListItem, Paragraph, Wrap};

pub struct UiState<'a> {
    pub lines: &'a [TranscriptLine],
    pub level: f32,
    pub language: &'a str,
    pub recording: bool,
    pub input_name: &'a str,
    pub model_name: &'a str,
    pub saved_note: Option<&'a str>,
    pub translator_status: TranslatorStatus,
    pub picker: Option<&'a DevicePicker>,
    pub draft: DraftState,
}

pub fn draw(f: &mut Frame, state: &UiState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(1),
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
        " kmark · {} · lang={} · in={} · model={} · {} ",
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

    let draft_text = if state.draft.active {
        format!(
            " ● capturing · {:.1}s speech · {:.1}s elapsed",
            state.draft.speech_ms as f32 / 1000.0,
            state.draft.elapsed_ms as f32 / 1000.0
        )
    } else {
        " (no speech)".to_string()
    };
    let draft_style = if state.draft.active {
        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let draft_para = Paragraph::new(draft_text).style(draft_style);
    f.render_widget(draft_para, chunks[1]);

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[2]);

    render_transcript_column(f, cols[0], state.lines, "en", " English ");
    render_transcript_column(f, cols[1], state.lines, "ja", " 日本語 ");

    let help = match state.saved_note {
        Some(note) => format!(
            " {} · q quit&save · s save · l lang · d device · space pause ",
            note
        ),
        None => " q quit&save · s save · l cycle lang · d device · space pause ".to_string(),
    };
    let help_p = Paragraph::new(help).style(Style::default().fg(Color::DarkGray));
    f.render_widget(help_p, chunks[3]);

    if let Some(pk) = state.picker {
        render_device_picker(f, pk);
    }
}

fn render_device_picker(f: &mut Frame, pk: &DevicePicker) {
    let area = centered_rect(60, 60, f.area());
    f.render_widget(Clear, area);
    let items: Vec<ListItem> = pk
        .devices
        .iter()
        .enumerate()
        .map(|(i, d)| {
            let marker = if i == pk.selected { "▶ " } else { "  " };
            let style = if i == pk.selected {
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(Span::styled(format!("{}{}", marker, d), style))
        })
        .collect();
    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Select input device  (↑↓ · enter · esc) "),
    );
    f.render_widget(list, area);
}

fn centered_rect(pct_x: u16, pct_y: u16, r: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - pct_y) / 2),
            Constraint::Percentage(pct_y),
            Constraint::Percentage((100 - pct_y) / 2),
        ])
        .split(r);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - pct_x) / 2),
            Constraint::Percentage(pct_x),
            Constraint::Percentage((100 - pct_x) / 2),
        ])
        .split(vertical[1])[1]
}

fn render_transcript_column(
    f: &mut Frame,
    area: Rect,
    lines: &[TranscriptLine],
    col_lang: &str,
    title: &str,
) {
    let block = Block::default().borders(Borders::ALL).title(title.to_string());
    let inner = block.inner(area);
    let wrapped: Vec<Line> = lines.iter().map(|l| render_line(l, col_lang)).collect();

    let para = Paragraph::new(wrapped).wrap(Wrap { trim: false });
    let total = para.line_count(inner.width) as u16;
    let scroll = total.saturating_sub(inner.height);
    let para = para.scroll((scroll, 0)).block(block);
    f.render_widget(para, area);
}

fn render_line<'a>(line: &'a TranscriptLine, col_lang: &str) -> Line<'a> {
    let ts = line.started_at.format("%H:%M:%S").to_string();
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
    let head = format!("[{}] {}", ts, marker);

    Line::from(vec![
        Span::styled(head, Style::default().fg(Color::Cyan)),
        Span::styled(text.to_string(), style),
    ])
}
