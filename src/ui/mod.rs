//! Top-level UI composition.

use crate::{
    app::state::{AppState, ReviewTab},
    domain::Route,
    render::markdown::MarkdownRenderer,
    ui::components::{
        footer,
        header::{self, HeaderModel, HeaderTabs, ReviewProgress},
    },
};
use ratatui::{
    Frame,
    layout::{Constraint, Layout},
};

pub mod components;
mod hints;
pub mod screens;
pub mod theme;

/// Draws the active screen.
pub fn render(frame: &mut Frame<'_>, state: &mut AppState, markdown: &mut MarkdownRenderer) {
    let hints = hints::build(state);

    let root = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(8),
        Constraint::Length(footer::required_height(frame.area().width, &hints)),
    ])
    .split(frame.area());

    let review_progress = state.review.as_ref().and_then(|review| {
        let (resolved, total) = review.data.review_thread_totals();
        (state.route == Route::Review && total > 0).then_some(ReviewProgress {
            resolved_threads: resolved,
            total_threads: total,
        })
    });

    let context_label = match state.route {
        Route::Search => state.repository_label.clone(),
        Route::Review => state.review.as_ref().map_or_else(
            || state.repository_label.clone(),
            |review| {
                format!(
                    "{}/{}#{}",
                    review.pull.owner, review.pull.repo, review.pull.number
                )
            },
        ),
    };
    let review_tabs = if state.route == Route::Review {
        state.review.as_ref().map(|review| HeaderTabs {
            selected: match review.active_tab() {
                ReviewTab::Threads => 0,
                ReviewTab::Diff => 1,
            },
        })
    } else {
        None
    };

    header::render(
        frame,
        root[0],
        &HeaderModel {
            app_label: "🔍 critic".to_owned(),
            context_label,
            viewer_login: state.viewer_login.clone(),
            review_tabs,
            operation: state.operation_display(),
            error: state.error_message.clone(),
            review_progress,
        },
    );

    match state.route {
        Route::Search => screens::search::render(frame, root[1], state),
        Route::Review => {
            if let Some(review) = state.review.as_mut() {
                screens::review::render(frame, root[1], review, markdown);
            } else {
                screens::search::render(frame, root[1], state);
            }
        }
    }

    footer::render(frame, root[2], &hints);
}
