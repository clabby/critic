//! Header component shared by search and review screens.

use crate::ui::theme;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph},
};

/// Header payload consumed by the renderer.
#[derive(Debug, Clone)]
pub struct HeaderModel {
    pub app_label: String,
    pub context_label: String,
    pub review_tabs: Option<HeaderTabs>,
    pub operation: Option<String>,
    pub error: Option<String>,
    pub review_progress: Option<ReviewProgress>,
}

/// Active tab indicator displayed in the header for review route.
#[derive(Debug, Clone, Copy)]
pub struct HeaderTabs {
    pub selected: usize,
}

/// Review thread progress stats for the selected pull request.
#[derive(Debug, Clone, Copy)]
pub struct ReviewProgress {
    pub resolved_threads: usize,
    pub total_threads: usize,
}

/// Renders the screen header with title, operation/error state, and thread progress.
pub fn render(frame: &mut Frame<'_>, area: Rect, model: &HeaderModel) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme::border());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut top_left_spans = vec![
        Span::styled(format!(" {}", model.app_label), theme::title()),
        Span::styled(format!(" {}", model.context_label), theme::dim()),
    ];
    if let Some(tabs) = model.review_tabs {
        top_left_spans.extend(review_tabs_spans(tabs));
    }
    if let Some(error) = &model.error {
        top_left_spans.push(Span::styled(format!("  error: {error}"), theme::error()));
    } else if let Some(operation) = &model.operation {
        top_left_spans.push(Span::styled(format!("  {operation}"), theme::info()));
    }
    let top_left = Line::from(top_left_spans);

    if let Some(progress) = model.review_progress {
        let right_width = inner.width.min(44);
        let columns =
            Layout::horizontal([Constraint::Min(1), Constraint::Length(right_width)]).split(inner);
        let right_sections = Layout::horizontal([
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(18),
        ])
        .split(columns[1]);

        frame.render_widget(Paragraph::new(top_left), columns[0]);
        frame.render_widget(
            Paragraph::new(Line::from(thread_ratio_text(progress))).alignment(Alignment::Right),
            right_sections[0],
        );
        frame.render_widget(Paragraph::new(" "), right_sections[1]);
        frame.render_widget(progress_gauge(progress), right_sections[2]);
    } else {
        frame.render_widget(Paragraph::new(top_left), inner);
    }
}

fn review_tabs_spans(tabs: HeaderTabs) -> [Span<'static>; 5] {
    let threads_style = if tabs.selected == 0 {
        theme::selected()
    } else {
        theme::dim()
    };
    let diff_style = if tabs.selected == 1 {
        theme::selected()
    } else {
        theme::dim()
    };

    [
        Span::raw("  "),
        Span::styled(" Threads ", threads_style),
        Span::styled(" | ", theme::dim()),
        Span::styled(" Diff ", diff_style),
        Span::raw(""),
    ]
}

fn thread_ratio_text(progress: ReviewProgress) -> String {
    format!(
        "Resolved Threads {}/{}",
        progress.resolved_threads, progress.total_threads
    )
}

fn progress_gauge(progress: ReviewProgress) -> Gauge<'static> {
    let ratio = if progress.total_threads == 0 {
        0.0
    } else {
        progress.resolved_threads as f64 / progress.total_threads as f64
    };
    let percent = (ratio * 100.0).round() as usize;

    Gauge::default()
        .ratio(ratio.clamp(0.0, 1.0))
        .label(Span::styled(format!("{percent}%"), theme::gauge_label()))
        .gauge_style(theme::gauge_fill())
        .style(theme::gauge_empty())
}
