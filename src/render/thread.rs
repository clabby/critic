//! Right-pane renderers for review threads and issue comments.

use crate::domain::{
    CommentRef, IssueComment, ListNode, ListNodeKind, PullReviewSummary, ReviewComment,
    ReviewThread, review_comment_is_outdated,
};
use crate::render::markdown::MarkdownRenderer;
use crate::ui::theme;
use ratatui::text::{Line, Span};

/// Renders the preview panel for a selected review thread node.
pub fn render_thread_preview(
    markdown: &mut MarkdownRenderer,
    selected_node: &ListNode,
    root_thread: &ReviewThread,
    reply_draft: Option<&str>,
) -> Vec<Line<'static>> {
    let mut out: Vec<Line<'static>> = Vec::new();

    let patch_comment =
        selected_patch_comment(selected_node, root_thread).unwrap_or(&root_thread.comment);
    append_patch_excerpt(&mut out, patch_comment);
    out.push(horizontal_rule());

    let status = if root_thread.is_resolved {
        "resolved"
    } else {
        "open"
    };
    let status_style = if root_thread.is_resolved {
        theme::resolved_thread()
    } else {
        theme::open_thread()
    };

    out.push(Line::from(vec![
        Span::styled("Thread", theme::section_title()),
        Span::raw(" "),
        Span::styled(format!("[{status}]"), status_style),
    ]));
    out.push(Line::default());

    render_thread_comment(markdown, &mut out, root_thread, 0);
    out.push(horizontal_rule());
    out.push(Line::from(vec![Span::styled(
        "Pending Reply",
        theme::section_title(),
    )]));

    let reply = reply_draft.unwrap_or("").trim();
    if reply.is_empty() {
        out.push(Line::from(vec![Span::styled(
            "  (empty)  [e] edit  [s] send  [x] clear",
            theme::dim(),
        )]));
    } else {
        let rendered = markdown.render(reply);
        out.extend(prefix_lines(rendered, "  "));
        out.push(Line::from(vec![Span::styled(
            "  [e] edit  [s] send  [x] clear",
            theme::dim(),
        )]));
    }

    out
}

/// Renders the preview panel for a selected issue comment.
pub fn render_issue_preview(
    markdown: &mut MarkdownRenderer,
    issue: &IssueComment,
) -> Vec<Line<'static>> {
    let mut out = Vec::new();

    out.push(Line::from(vec![Span::styled(
        "Issue Comment",
        theme::section_title(),
    )]));
    out.push(Line::from(vec![Span::styled(
        format!(
            "@{}  {}",
            issue.user.login,
            short_date(issue.created_at.to_rfc3339().as_str())
        ),
        theme::dim(),
    )]));
    out.push(Line::default());

    let rendered = markdown.render(issue.body.as_deref().unwrap_or(""));
    out.extend(prefix_lines(rendered, "  "));

    out
}

fn render_thread_comment(
    markdown: &mut MarkdownRenderer,
    out: &mut Vec<Line<'static>>,
    thread: &ReviewThread,
    depth: usize,
) {
    let indent = "  ".repeat(depth);

    out.push(Line::from(vec![
        Span::styled(
            format!(
                "{indent}@{}",
                thread
                    .comment
                    .user
                    .as_ref()
                    .map(|user| user.login.as_str())
                    .unwrap_or("unknown")
            ),
            theme::author(),
        ),
        Span::raw("  "),
        Span::styled(
            short_date(thread.comment.created_at.to_rfc3339().as_str()),
            theme::dim(),
        ),
    ]));

    let rendered = markdown.render(thread.comment.body.as_str());
    out.extend(prefix_lines(rendered, &format!("{indent}  ")));
    out.push(Line::default());

    for reply in &thread.replies {
        render_thread_comment(markdown, out, reply, depth + 1);
    }
}

fn selected_patch_comment<'a>(
    selected: &'a ListNode,
    root: &'a ReviewThread,
) -> Option<&'a ReviewComment> {
    match &selected.comment {
        CommentRef::Review(comment) if !comment.diff_hunk.trim().is_empty() => Some(comment),
        _ => first_patch_comment(root),
    }
}

fn first_patch_comment(thread: &ReviewThread) -> Option<&ReviewComment> {
    if !thread.comment.diff_hunk.trim().is_empty() {
        return Some(&thread.comment);
    }

    for reply in &thread.replies {
        if let Some(found) = first_patch_comment(reply) {
            return Some(found);
        }
    }

    None
}

fn append_patch_excerpt(out: &mut Vec<Line<'static>>, comment: &ReviewComment) {
    let outdated = review_comment_is_outdated(comment);
    out.push(Line::from(vec![
        Span::styled("Patch", theme::section_title()),
        Span::raw("  "),
        Span::styled(comment_location(comment), theme::dim()),
        if outdated {
            Span::styled("  [outdated]", theme::outdated())
        } else {
            Span::raw("")
        },
    ]));

    if comment.diff_hunk.trim().is_empty() {
        out.push(Line::from(vec![Span::styled(
            "  [no patch hunk available]",
            theme::dim(),
        )]));
        return;
    }

    for line in comment.diff_hunk.lines().take(28) {
        let style = if line.starts_with("@@") {
            theme::diff_header()
        } else if line.starts_with('+') {
            theme::diff_add()
        } else if line.starts_with('-') {
            theme::diff_remove()
        } else {
            theme::diff_context()
        };

        out.push(Line::from(vec![Span::styled(format!("  {line}"), style)]));
    }
}

fn comment_location(comment: &ReviewComment) -> String {
    let path = if comment.path.trim().is_empty() {
        "(unknown path)"
    } else {
        comment.path.as_str()
    };
    match (comment.start_line, comment.line) {
        (Some(start), Some(end)) if start != end => format!("{path}:{start}-{end}"),
        (_, Some(line)) => format!("{path}:{line}"),
        _ => path.to_owned(),
    }
}

/// Renders pull-request review summary content from `/pulls/{pull}/reviews`.
pub fn render_review_summary_preview(
    markdown: &mut MarkdownRenderer,
    review: &PullReviewSummary,
) -> Vec<Line<'static>> {
    let mut out = Vec::new();

    out.push(Line::from(vec![Span::styled(
        "Review Summary",
        theme::section_title(),
    )]));
    out.push(Line::from(vec![Span::styled(
        format!(
            "@{}  {}",
            review
                .user
                .as_ref()
                .map(|user| user.login.as_str())
                .unwrap_or("unknown"),
            review
                .submitted_at
                .map(|value| short_date(value.to_rfc3339().as_str()))
                .unwrap_or_else(|| "unknown".to_owned())
        ),
        theme::dim(),
    )]));
    out.push(Line::default());

    let rendered = markdown.render(review.body.as_deref().unwrap_or(""));
    out.extend(prefix_lines(rendered, "  "));

    out
}

fn prefix_lines(lines: Vec<Line<'static>>, prefix: &str) -> Vec<Line<'static>> {
    lines
        .into_iter()
        .map(|line| {
            let mut spans = Vec::with_capacity(line.spans.len() + 1);
            spans.push(Span::styled(prefix.to_owned(), theme::dim()));
            spans.extend(line.spans);
            Line::from(spans)
        })
        .collect()
}

fn short_date(value: &str) -> String {
    if value.len() >= 16 {
        return value[..16].replace('T', " ");
    }
    value.to_owned()
}

fn horizontal_rule() -> Line<'static> {
    Line::from(vec![Span::styled(
        "────────────────────────────────────────────────────────────────────────────",
        theme::dim(),
    )])
}

#[allow(dead_code)]
fn is_thread_like(node: &ListNode) -> bool {
    matches!(node.kind, ListNodeKind::Thread | ListNodeKind::Reply)
}
