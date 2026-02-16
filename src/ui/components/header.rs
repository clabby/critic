//! Header component shared by search and review screens.

use crate::ui::theme;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

/// Header payload consumed by the renderer.
#[derive(Debug, Clone)]
pub struct HeaderModel {
    pub app_label: String,
    pub context_label: String,
    pub hints: String,
    pub operation: Option<String>,
    pub error: Option<String>,
    pub resolved_threads: usize,
    pub total_threads: usize,
}

/// Renders the screen header with title, progress, and keybinding hints.
pub fn render(frame: &mut Frame<'_>, area: Rect, model: &HeaderModel) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme::border());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let rows = Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).split(inner);

    let ratio_text = thread_ratio_text(model.resolved_threads, model.total_threads);
    let progress = progress_bar(model.resolved_threads, model.total_threads, 18);

    let top = Line::from(vec![
        Span::styled(format!(" {}", model.app_label), theme::title()),
        Span::styled(format!(" {}", model.context_label), theme::dim()),
        Span::raw("    "),
        Span::styled(
            ratio_text,
            Style::default().fg(theme::title().fg.unwrap_or_default()),
        ),
        Span::raw(" "),
        Span::styled(progress, theme::info()),
        if let Some(operation) = &model.operation {
            Span::styled(format!("  {operation}"), theme::info())
        } else {
            Span::raw("")
        },
    ]);

    let hints = if let Some(error) = &model.error {
        Line::from(vec![Span::styled(
            format!(" error: {error}"),
            theme::error(),
        )])
    } else {
        Line::from(vec![Span::styled(
            format!(" {}", model.hints),
            theme::dim(),
        )])
    };

    frame.render_widget(Paragraph::new(top), rows[0]);
    frame.render_widget(Paragraph::new(hints), rows[1]);
}

fn thread_ratio_text(resolved: usize, total: usize) -> String {
    if total == 0 {
        "Review Threads 0/0".to_owned()
    } else {
        let percent = ((resolved as f64 / total as f64) * 100.0).round() as usize;
        format!("Review Threads {resolved}/{total} ({percent}%)")
    }
}

fn progress_bar(resolved: usize, total: usize, width: usize) -> String {
    if total == 0 || width == 0 {
        return String::new();
    }

    let ratio = resolved as f64 / total as f64;
    let filled = (ratio * width as f64).round() as usize;

    let mut out = String::with_capacity(width);
    for i in 0..width {
        if i < filled {
            out.push('█');
        } else {
            out.push('░');
        }
    }
    out
}
