//! Footer hint composition for each route and interaction mode.

use crate::{
    app::state::{AppState, ReviewScreenState, ReviewTab},
    domain::{CommentRef, ListNodeKind, Route},
};

pub fn build(state: &AppState) -> String {
    match state.route {
        Route::Search => search_hints(state),
        Route::Review => review_hints(state),
    }
}

fn search_hints(state: &AppState) -> String {
    if state.is_search_focused() {
        "[type] edit query  [backspace] delete  [enter/esc] unfocus".to_owned()
    } else {
        "[j/k/up/down] navigate  [enter] open PR  [W] open web  [s] focus search  [R] refresh  [q] quit"
            .to_owned()
    }
}

fn review_hints(state: &AppState) -> String {
    let Some(review) = state.review.as_ref() else {
        return fallback_review_hints(state);
    };

    match review.active_tab() {
        ReviewTab::Diff => review_diff_hints(review),
        ReviewTab::Threads => review_thread_hints(review),
    }
}

fn review_diff_hints(review: &ReviewScreenState) -> String {
    if review.is_diff_search_focused() {
        return "[type] edit file filter  [backspace] delete  [enter/esc] unfocus".to_owned();
    }

    let is_visual_mode = review.has_diff_selection_anchor();
    let mut parts = Vec::new();

    if !is_visual_mode {
        parts.push("[S-tab] show threads".to_owned());
        if review.is_diff_content_focused() {
            parts.push("[tab] focus files".to_owned());
        } else {
            parts.push("[tab] focus diff".to_owned());
        }
    }

    parts.push("[j/k/up/down] navigate".to_owned());
    if !is_visual_mode {
        parts.push("[n/N] next/prev hunk".to_owned());
        if review.pending_review_comment_count() > 0 {
            parts.push("[p/P] pending next/prev".to_owned());
        }
    }
    if review.is_diff_content_focused() {
        parts.push("[C-d/C-u] scroll paragraph".to_owned());
    }

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

    parts.join("  ")
}

fn review_thread_hints(review: &ReviewScreenState) -> String {
    if review.is_thread_search_focused() {
        return "[type] edit comment filter  [backspace] delete  [enter/esc] unfocus".to_owned();
    }

    let mut parts = vec![
        "[S-tab] show diff".to_owned(),
        "[j/k/up/down] navigate".to_owned(),
        "[C-d/C-u] scroll paragraph".to_owned(),
    ];

    if review.data.review_thread_totals().1 > 0 {
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
        if has_openable_comment_url(&node.comment) {
            parts.push("[W] open web".to_owned());
        }
    }

    let has_sendable_reply = review
        .selected_reply_draft()
        .is_some_and(|draft| !draft.trim().is_empty());

    if let Some(context) = review.selected_thread_context() {
        if context.thread_id.is_some() {
            let thread_action = if context.is_resolved {
                "[t] unresolve"
            } else {
                "[t] resolve"
            };
            parts.push(thread_action.to_owned());
        }

        if has_sendable_reply {
            parts.push("[e/s/x] reply".to_owned());
        } else {
            parts.push("[e] edit reply".to_owned());
        }
    }

    if has_sendable_reply {
        parts.push("[/] search comments".to_owned());
    } else {
        parts.push("[s] search comments".to_owned());
    }

    if review.pending_review_comment_count() > 0 {
        parts.push("[C/A/X] submit review (+pending)".to_owned());
    } else {
        parts.push("[C/A/X] review submit".to_owned());
    }
    parts.push("[b] back".to_owned());
    parts.push("[R] refresh".to_owned());
    parts.push("[q] quit".to_owned());

    parts.join("  ")
}

fn fallback_review_hints(state: &AppState) -> String {
    let mut parts = vec!["[S-tab] show diff".to_owned()];
    if state
        .review
        .as_ref()
        .is_some_and(|review| review.pending_review_comment_count() > 0)
    {
        parts.push("[C/A/X] submit review (+pending)".to_owned());
    } else {
        parts.push("[C/A/X] review submit".to_owned());
    }
    parts.push("[b] back".to_owned());
    parts.push("[R] refresh".to_owned());
    parts.push("[q] quit".to_owned());
    parts.join("  ")
}

fn has_openable_comment_url(comment: &CommentRef) -> bool {
    match comment {
        CommentRef::Review(comment) => !comment.html_url.trim().is_empty(),
        CommentRef::Issue(comment) => !comment.html_url.as_str().trim().is_empty(),
        CommentRef::ReviewSummary(review) => !review.html_url.as_str().trim().is_empty(),
    }
}
