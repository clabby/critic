//! Top-level UI composition.

use crate::app::state::AppState;
use crate::app::state::ReviewTab;
use crate::domain::{CommentRef, ListNodeKind, Route};
use crate::render::markdown::MarkdownRenderer;
use crate::ui::components::footer;
use crate::ui::components::header::{self, HeaderModel, HeaderTabs, ReviewProgress};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};

pub mod components;
pub mod screens;
pub mod theme;

/// Draws the active screen.
pub fn render(frame: &mut Frame<'_>, state: &mut AppState, markdown: &mut MarkdownRenderer) {
    let hints = build_hints(state);

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

fn build_hints(state: &AppState) -> String {
    match state.route {
        Route::Search => {
            if state.is_search_focused() {
                "[type] edit query  [backspace] delete  [enter/esc] unfocus".to_owned()
            } else {
                "[j/k/up/down] navigate  [enter] open PR  [W] open web  [s] focus search  [R] refresh  [q] quit"
                    .to_owned()
            }
        }
        Route::Review => {
            if let Some(review) = state.review.as_ref() {
                let is_visual_mode =
                    review.active_tab() == ReviewTab::Diff && review.has_diff_selection_anchor();
                let mut parts = Vec::new();
                if !is_visual_mode {
                    let tab_hint = if review.active_tab() == ReviewTab::Diff {
                        "[S-tab] show threads"
                    } else {
                        "[S-tab] show diff"
                    };
                    parts.push(tab_hint.to_owned());
                    if review.active_tab() == ReviewTab::Diff {
                        if review.is_diff_content_focused() {
                            parts.push("[tab] focus files".to_owned());
                        } else {
                            parts.push("[tab] focus diff".to_owned());
                        }
                    }
                }

                if review.active_tab() == ReviewTab::Diff {
                    if review.is_diff_search_focused() {
                        return "[type] edit file filter  [backspace] delete  [enter/esc] unfocus"
                            .to_owned();
                    }

                    parts.push("[j/k/up/down] navigate".to_owned());
                    if !is_visual_mode {
                        parts.push("[n/N] next/prev hunk".to_owned());
                        if review.pending_review_comment_count() > 0 {
                            parts.push("[p/P] pending next/prev".to_owned());
                        }
                    }
                    parts.push("[C-d/C-u] scroll paragraph".to_owned());
                    if review.is_diff_content_focused() {
                        parts.push("[v] range".to_owned());
                        if review.selected_diff_range().is_some() {
                            parts.push("[esc] cancel visual".to_owned());
                            parts.push("[e] leave comment".to_owned());
                        } else if review.selected_pending_review_comment().is_some() {
                            parts.push("[e/x] edit or delete pending".to_owned());
                        } else {
                            parts.push("[e] leave comment".to_owned());
                        }
                    } else {
                        parts.push("[s] search files".to_owned());
                        parts.push("[o/z] collapse".to_owned());
                    }
                    if review.pending_review_comment_count() > 0 && !is_visual_mode {
                        parts.push("[C/A/X] submit review".to_owned());
                    }
                    if !is_visual_mode {
                        parts.push("[b] back".to_owned());
                        parts.push("[R] refresh".to_owned());
                    }
                    parts.push("[q] quit".to_owned());
                    return parts.join("  ");
                }

                parts.push("[j/k/up/down] navigate".to_owned());

                let has_review_threads = review.data.review_thread_totals().1 > 0;
                if has_review_threads {
                    let resolved_hint = if review.hide_resolved {
                        "[f] show resolved"
                    } else {
                        "[f] hide resolved"
                    };
                    parts.push(resolved_hint.to_owned());
                }

                if let Some(node) = review.selected_node() {
                    let collapsible_review_group =
                        node.kind == ListNodeKind::Review && node.key.starts_with("review-group:");
                    if node.kind == ListNodeKind::Thread || collapsible_review_group {
                        parts.push("[o/z] collapse".to_owned());
                    }
                    let can_open_web = match &node.comment {
                        CommentRef::Review(comment) => !comment.html_url.trim().is_empty(),
                        CommentRef::Issue(comment) => !comment.html_url.as_str().trim().is_empty(),
                        CommentRef::ReviewSummary(review) => {
                            !review.html_url.as_str().trim().is_empty()
                        }
                    };
                    if can_open_web {
                        parts.push("[W] open web".to_owned());
                    }
                }

                if let Some(context) = review.selected_thread_context() {
                    if context.thread_id.is_some() {
                        let thread_action = if context.is_resolved {
                            "[t] unresolve"
                        } else {
                            "[t] resolve"
                        };
                        parts.push(thread_action.to_owned());
                    }

                    let has_reply_draft = review
                        .selected_reply_draft()
                        .map(|draft| !draft.trim().is_empty())
                        .unwrap_or(false);
                    if has_reply_draft {
                        parts.push("[e/s/x] reply".to_owned());
                    } else {
                        parts.push("[e] edit reply".to_owned());
                    }
                }

                let submit_hint = if review.pending_review_comment_count() > 0 {
                    "[C/A/X] submit review (+pending)"
                } else {
                    "[C/A/X] review submit"
                };
                parts.push(submit_hint.to_owned());
                parts.push("[b] back".to_owned());
                parts.push("[R] refresh".to_owned());
                parts.push("[q] quit".to_owned());

                return parts.join("  ");
            }

            let mut parts = vec!["[S-tab] show diff".to_owned()];
            let submit_hint = if state
                .review
                .as_ref()
                .is_some_and(|review| review.pending_review_comment_count() > 0)
            {
                "[C/A/X] submit review (+pending)"
            } else {
                "[C/A/X] review submit"
            };
            parts.push(submit_hint.to_owned());
            parts.push("[b] back".to_owned());
            parts.push("[R] refresh".to_owned());
            parts.push("[q] quit".to_owned());

            parts.join("  ")
        }
    }
}
