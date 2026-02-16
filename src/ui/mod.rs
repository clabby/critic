//! Top-level UI composition.

use crate::app::state::{AppState, InputState};
use crate::domain::Route;
use crate::render::markdown::MarkdownRenderer;
use crate::ui::components::header::{self, HeaderModel};
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

pub mod components;
pub mod screens;
pub mod theme;

/// Draws the active screen.
pub fn render(frame: &mut Frame<'_>, state: &AppState, markdown: &mut MarkdownRenderer) {
    let root = Layout::vertical([Constraint::Length(4), Constraint::Min(8)]).split(frame.area());

    let (resolved, total) = state
        .review
        .as_ref()
        .map(|review| review.data.review_thread_totals())
        .unwrap_or((0, 0));

    let context_label = match state.route {
        Route::Search => state.repository_label.clone(),
        Route::Review => state
            .review
            .as_ref()
            .map(|review| {
                format!(
                    "{}/{}#{}",
                    review.pull.owner, review.pull.repo, review.pull.number
                )
            })
            .unwrap_or_else(|| state.repository_label.clone()),
    };

    let hints = match state.route {
        Route::Search => {
            "[j/k/up/down] navigate  [enter] open PR  [type] fuzzy filter  [r] refresh  [q] quit"
                .to_owned()
        }
        Route::Review => {
            let hide_resolved = state
                .review
                .as_ref()
                .map(|review| review.hide_resolved)
                .unwrap_or(true);
            let resolved_hint = if hide_resolved {
                "[f] show resolved"
            } else {
                "[f] hide resolved"
            };
            format!(
                "[j/k/up/down] navigate  [o/z] collapse  [t] resolve/unresolve  {resolved_hint}  [e/s/x] reply  [C/A/X] review submit  [pgup/pgdn] scroll  [b] back  [r] refresh  [q] quit"
            )
        }
    };

    header::render(
        frame,
        root[0],
        &HeaderModel {
            app_label: "review-tui".to_owned(),
            context_label,
            hints,
            operation: state.operation_display(),
            error: state.error_message.clone(),
            resolved_threads: resolved,
            total_threads: total,
        },
    );

    match state.route {
        Route::Search => screens::search::render(frame, root[1], state),
        Route::Review => screens::review::render(frame, root[1], state, markdown),
    }

    if let Some(input) = state.input.as_ref() {
        render_input_overlay(frame, input);
    }
}

fn render_input_overlay(frame: &mut Frame<'_>, input: &InputState) {
    let area = centered_rect(70, 24, frame.area());
    let block = Block::default()
        .title(format!(" {} ", input.title))
        .borders(Borders::ALL);

    let lines = vec![
        Line::from(format!(" {}", input.prompt)),
        Line::from(""),
        Line::from(format!(" > {}", input.buffer)),
        Line::from(""),
        Line::from(" Enter submit   Esc cancel"),
    ];

    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false })
            .alignment(Alignment::Left),
        area,
    );
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(area);

    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(popup_layout[1])[1]
}
