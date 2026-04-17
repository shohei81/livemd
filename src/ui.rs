use crate::app::DevicePicker;
use crate::msg::TranslatorStatus;
use crate::transcribe::TranscriptLine;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Gauge, List, ListItem, Paragraph};

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
            " {} · q quit&save · s save · l lang · d device · space pause ",
            note
        ),
        None => " q quit&save · s save · l cycle lang · d device · space pause ".to_string(),
    };
    let help_p = Paragraph::new(help).style(Style::default().fg(Color::DarkGray));
    f.render_widget(help_p, chunks[2]);

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

fn render_cell<'a>(line: &'a TranscriptLine, col_lang: &str) -> ListItem<'a> {
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

    ListItem::new(Line::from(vec![
        Span::styled(head, Style::default().fg(Color::Cyan)),
        Span::styled(text.to_string(), style),
    ]))
}
