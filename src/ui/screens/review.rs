//! Review screen renderer with tabs for threads and diffs.

use crate::app::state::{PendingReviewCommentDraft, ReviewScreenState, ReviewTab};
use crate::domain::{CommentRef, ListNodeKind, PullRequestDiffFileStatus};
use crate::render::markdown::MarkdownRenderer;
use crate::render::thread::{
    render_issue_preview, render_review_summary_preview, render_thread_preview,
};
use crate::ui::components::shared::short_preview;
use crate::ui::screens::review_diff::{self, DiffRowsRenderContext};
use crate::ui::theme;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, List, ListItem, ListState, Paragraph, Scrollbar, ScrollbarOrientation,
    ScrollbarState, Wrap,
};
use std::collections::HashMap;

pub fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    review: &mut ReviewScreenState,
    markdown: &mut MarkdownRenderer,
) {
    match review.active_tab() {
        ReviewTab::Threads => render_threads_tab(frame, area, review, markdown),
        ReviewTab::Diff => render_diff_tab(frame, area, review, markdown),
    }
}

fn render_threads_tab(
    frame: &mut Frame<'_>,
    area: Rect,
    review: &ReviewScreenState,
    markdown: &mut MarkdownRenderer,
) {
    let panes =
        Layout::horizontal([Constraint::Percentage(47), Constraint::Percentage(53)]).split(area);

    render_left_pane(frame, panes[0], review);
    render_right_pane(frame, panes[1], review, markdown);
}

fn render_diff_tab(
    frame: &mut Frame<'_>,
    area: Rect,
    review: &mut ReviewScreenState,
    markdown: &mut MarkdownRenderer,
) {
    let panes =
        Layout::horizontal([Constraint::Percentage(15), Constraint::Percentage(85)]).split(area);

    render_diff_files(frame, panes[0], review);
    render_diff_content(frame, panes[1], review, markdown);
}

fn render_left_pane(frame: &mut Frame<'_>, area: Rect, review: &ReviewScreenState) {
    let pending_count = review.pending_review_comment_count();
    let sections = if pending_count > 0 {
        let pending_height = (pending_count.min(3) as u16).saturating_add(2);
        Layout::vertical([Constraint::Length(pending_height), Constraint::Min(4)]).split(area)
    } else {
        Layout::vertical([Constraint::Min(1)]).split(area)
    };

    if pending_count > 0 {
        render_pending_review_sidebar(frame, sections[0], review);
    }

    let comments_area = *sections.last().unwrap_or(&area);
    let block = Block::default()
        .title(Span::styled(
            format!(" Comments ({}) ", review.nodes.len()),
            theme::title(),
        ))
        .borders(Borders::ALL)
        .border_style(theme::border());

    let items: Vec<ListItem<'static>> = if review.nodes.is_empty() {
        vec![ListItem::new(Line::from(vec![Span::styled(
            "No comments available.",
            theme::dim(),
        )]))]
    } else {
        review
            .nodes
            .iter()
            .map(|node| {
                let indent = "  ".repeat(node.depth);

                match node.kind {
                    ListNodeKind::Thread => {
                        let preview = short_preview(node.comment.body(), 58);
                        let icon = if review.is_collapsed(&node.key) {
                            "▸"
                        } else {
                            "▾"
                        };
                        let status = if node.is_resolved { "resolved" } else { "open" };
                        let status_style = if node.is_resolved {
                            theme::resolved_thread()
                        } else {
                            theme::open_thread()
                        };
                        ListItem::new(Line::from(vec![
                            Span::styled(
                                format!("{indent}{icon} @{} ", node.comment.author()),
                                theme::title(),
                            ),
                            if node.is_outdated {
                                Span::styled("[outdated] ", theme::open_thread())
                            } else {
                                Span::raw("")
                            },
                            Span::raw(preview),
                            Span::raw("  "),
                            Span::styled(format!("[{status}]"), status_style),
                        ]))
                    }
                    ListNodeKind::Reply => {
                        let preview = short_preview(node.comment.body(), 64);
                        ListItem::new(Line::from(vec![
                            Span::styled(
                                format!("{indent}↳ @{} ", node.comment.author()),
                                theme::dim(),
                            ),
                            Span::styled(preview, theme::dim()),
                        ]))
                    }
                    ListNodeKind::Issue => ListItem::new(Line::from(vec![
                        Span::styled(
                            format!("{indent}• @{} ", node.comment.author()),
                            theme::issue(),
                        ),
                        Span::raw(short_preview(node.comment.body(), 58)),
                        Span::raw("  "),
                        Span::styled("issue", theme::issue()),
                    ])),
                    ListNodeKind::Review => {
                        if node.key.starts_with("review-group:") {
                            let icon = if review.is_collapsed(&node.key) {
                                "▸"
                            } else {
                                "▾"
                            };
                            let status = if node.is_resolved { "resolved" } else { "open" };
                            let status_style = if node.is_resolved {
                                theme::resolved_thread()
                            } else {
                                theme::open_thread()
                            };

                            ListItem::new(Line::from(vec![
                                Span::styled(
                                    format!("{indent}{icon} @{} ", node.comment.author()),
                                    theme::title(),
                                ),
                                Span::raw(short_preview(node.comment.body(), 56)),
                                Span::raw("  "),
                                Span::styled(format!("[{status}]"), status_style),
                            ]))
                        } else {
                            ListItem::new(Line::from(vec![
                                Span::styled(
                                    format!("{indent}• @{} ", node.comment.author()),
                                    theme::issue(),
                                ),
                                Span::raw(short_preview(node.comment.body(), 58)),
                                Span::raw("  "),
                                Span::styled("review", theme::issue()),
                            ]))
                        }
                    }
                }
            })
            .collect()
    };

    let list = List::new(items)
        .block(block)
        .highlight_style(theme::selected())
        .highlight_symbol("▌ ");

    let mut list_state = ListState::default();
    if !review.nodes.is_empty() {
        list_state.select(Some(review.selected_row));
    }

    frame.render_stateful_widget(list, comments_area, &mut list_state);
}

fn render_pending_review_sidebar(frame: &mut Frame<'_>, area: Rect, review: &ReviewScreenState) {
    let pending = review.pending_review_comments();
    let block = Block::default()
        .title(Span::styled(
            format!(" Pending Review Comments ({}) ", pending.len()),
            theme::title(),
        ))
        .borders(Borders::ALL)
        .border_style(theme::border());
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let visible = usize::from(inner.height);
    if visible == 0 {
        return;
    }

    let lines = pending
        .iter()
        .take(visible)
        .map(|comment| {
            let location = if let Some(start) = comment.start_line {
                format!("{}:{}-{}", comment.path, start, comment.line)
            } else {
                format!("{}:{}", comment.path, comment.line)
            };
            let preview = short_preview(&comment.body, 42);
            let outdated = review.pending_review_comment_is_outdated(comment);
            Line::from(vec![
                Span::styled("• ", theme::open_thread()),
                Span::styled(location, theme::dim()),
                if outdated {
                    Span::styled(" [outdated]", theme::error())
                } else {
                    Span::raw("")
                },
                Span::raw(" "),
                Span::raw(preview),
            ])
        })
        .collect::<Vec<_>>();
    frame.render_widget(Paragraph::new(lines), inner);
}

fn render_right_pane(
    frame: &mut Frame<'_>,
    area: Rect,
    review: &ReviewScreenState,
    markdown: &mut MarkdownRenderer,
) {
    let block = Block::default()
        .title(Span::styled(" Thread Preview ", theme::title()))
        .borders(Borders::ALL)
        .border_style(theme::border());

    let lines = if let Some(node) = review.selected_node() {
        match (&node.kind, &node.comment) {
            (ListNodeKind::Issue, CommentRef::Issue(issue)) => {
                render_issue_preview(markdown, issue)
            }
            (ListNodeKind::Issue, CommentRef::ReviewSummary(review)) => {
                render_review_summary_preview(markdown, review)
            }
            (ListNodeKind::Review, CommentRef::ReviewSummary(review)) => {
                render_review_summary_preview(markdown, review)
            }
            (_, CommentRef::Review(_)) => {
                if let Some(root) = review.selected_root_thread() {
                    render_thread_preview(markdown, node, root, review.selected_reply_draft())
                } else {
                    vec![Line::from(vec![Span::styled(
                        "Thread not found for selected row.",
                        theme::error(),
                    )])]
                }
            }
            _ => vec![Line::from("Select a row to preview comment details.")],
        }
    } else {
        vec![Line::from("Select a row to preview comment details.")]
    };

    let inner = block.inner(area);
    frame.render_widget(block, area);
    let (text_area, scrollbar_area) = if inner.width > 1 {
        let sections = Layout::horizontal([Constraint::Min(1), Constraint::Length(1)]).split(inner);
        (sections[0], Some(sections[1]))
    } else {
        (inner, None)
    };

    let viewport_height = usize::from(text_area.height);
    let content_height = wrapped_content_height(&lines, text_area.width);
    let max_scroll = content_height.saturating_sub(viewport_height);
    let scroll = usize::from(review.right_scroll).min(max_scroll);

    let paragraph = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((u16::try_from(scroll).unwrap_or(u16::MAX), 0));

    frame.render_widget(paragraph, text_area);

    if content_height > viewport_height
        && let Some(area) = scrollbar_area
    {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None)
            .track_style(theme::dim())
            .thumb_style(theme::title());

        let scroll_positions = max_scroll.saturating_add(1);
        let mut scrollbar_state = ScrollbarState::new(scroll_positions)
            .viewport_content_length(viewport_height)
            .position(scroll);

        frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
    }
}

fn render_diff_files(frame: &mut Frame<'_>, area: Rect, review: &ReviewScreenState) {
    let file_count = review.diff_file_count();
    let rows = review.diff_tree_rows();
    let row_count = rows.len();
    let border_style = if review.is_diff_content_focused() {
        theme::border()
    } else {
        theme::open_thread()
    };
    let block = Block::default()
        .title(Span::styled(
            format!(" Files ({file_count}) ",),
            theme::title(),
        ))
        .borders(Borders::ALL)
        .border_style(border_style);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut constraints = vec![Constraint::Length(3)];
    if review.pending_review_comment_count() > 0 {
        constraints.push(Constraint::Length(5));
    }
    constraints.push(Constraint::Min(0));
    let sections = Layout::vertical(constraints).split(inner);
    let search_area = sections[0];
    let (pending_area, body_area) = if review.pending_review_comment_count() > 0 {
        (Some(sections[1]), sections[2])
    } else {
        (None, sections[1])
    };

    let focused = review.is_diff_search_focused();
    let title_style = if focused {
        theme::info()
    } else {
        theme::title()
    };
    let search_block = Block::default()
        .title(Span::styled(" File Search ", title_style))
        .borders(Borders::ALL)
        .border_style(if focused {
            theme::open_thread()
        } else {
            theme::border()
        });
    let search_line = if review.diff_search_query().is_empty() {
        Line::from(vec![
            Span::raw("  query: "),
            Span::styled(
                if focused {
                    "(type to filter changed files)"
                } else if review.is_diff_content_focused() {
                    "(focus files to search)"
                } else {
                    "(press [s] to search files)"
                },
                theme::dim(),
            ),
        ])
    } else {
        let mut line = vec![
            Span::raw("  query: "),
            Span::styled(review.diff_search_query().to_owned(), theme::text()),
        ];
        if focused {
            line.push(Span::styled(" |", theme::open_thread()));
        }
        Line::from(line)
    };
    let search_widget = Paragraph::new(search_line).block(search_block);
    frame.render_widget(search_widget, search_area);

    if let Some(pending_area) = pending_area {
        render_pending_review_sidebar(frame, pending_area, review);
    }

    let (list_area, scrollbar_area) = if body_area.width > 1 {
        let columns =
            Layout::horizontal([Constraint::Min(1), Constraint::Length(1)]).split(body_area);
        (columns[0], Some(columns[1]))
    } else {
        (body_area, None)
    };

    let items: Vec<ListItem<'static>> = if let Some(diff) = &review.diff {
        let mut pending_by_path = HashMap::<String, usize>::new();
        for draft in review.pending_review_comments() {
            *pending_by_path.entry(draft.path.clone()).or_default() += 1;
        }

        if rows.is_empty() {
            vec![ListItem::new(Line::from(vec![Span::styled(
                "No changed files.",
                theme::dim(),
            )]))]
        } else {
            rows.iter()
                .map(|row| {
                    let indent = "  ".repeat(row.depth);
                    if row.is_directory {
                        let icon = if row.is_collapsed { "▸ " } else { "▾ " };
                        return ListItem::new(Line::from(vec![Span::styled(
                            format!("{indent}{icon}{}/", row.label),
                            theme::title(),
                        )]));
                    }

                    let status = row
                        .file_index
                        .and_then(|index| diff.files.get(index))
                        .map(|file| match file.status {
                            PullRequestDiffFileStatus::Added => {
                                Span::styled("[A] ", theme::resolved_thread())
                            }
                            PullRequestDiffFileStatus::Removed => {
                                Span::styled("[D] ", theme::error())
                            }
                            PullRequestDiffFileStatus::Modified => {
                                Span::styled("[M] ", theme::title())
                            }
                        })
                        .unwrap_or_else(|| Span::styled("[?] ", theme::dim()));

                    ListItem::new(Line::from(vec![
                        Span::styled(indent.to_string(), theme::dim()),
                        status,
                        row.file_index
                            .and_then(|index| diff.files.get(index))
                            .and_then(|file| pending_by_path.get(&file.path).copied())
                            .filter(|count| *count > 0)
                            .map(|count| {
                                if count > 1 {
                                    Span::styled(format!("●{count} "), theme::open_thread())
                                } else {
                                    Span::styled("● ".to_owned(), theme::open_thread())
                                }
                            })
                            .unwrap_or_else(|| Span::raw("")),
                        Span::raw(row.label.clone()),
                    ]))
                })
                .collect()
        }
    } else if let Some(error) = &review.diff_error {
        vec![ListItem::new(Line::from(vec![Span::styled(
            format!("Diff unavailable: {error}"),
            theme::error(),
        )]))]
    } else {
        vec![ListItem::new(Line::from(vec![Span::styled(
            "Loading pull request diff...",
            theme::dim(),
        )]))]
    };

    let mut list_state = ListState::default();
    if row_count > 0 {
        list_state.select(Some(
            review
                .selected_diff_tree_row()
                .min(row_count.saturating_sub(1)),
        ));
    }

    let list = List::new(items)
        .highlight_style(theme::selected())
        .highlight_symbol("▸ ");
    frame.render_stateful_widget(list, list_area, &mut list_state);

    if list_area.height > 0
        && row_count > usize::from(list_area.height)
        && let Some(scrollbar_area) = scrollbar_area
    {
        let max_scroll = row_count.saturating_sub(usize::from(list_area.height));
        let scroll = list_state.offset().min(max_scroll);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None)
            .track_style(theme::dim())
            .thumb_style(theme::title());
        let mut scrollbar_state = ScrollbarState::new(max_scroll.saturating_add(1))
            .viewport_content_length(usize::from(list_area.height))
            .position(scroll);
        frame.render_stateful_widget(scrollbar, scrollbar_area, &mut scrollbar_state);
    }
}

fn render_diff_content(
    frame: &mut Frame<'_>,
    area: Rect,
    review: &mut ReviewScreenState,
    markdown: &mut MarkdownRenderer,
) {
    let border_style = if review.is_diff_content_focused() {
        theme::open_thread()
    } else {
        theme::border()
    };
    let pending_count = review.pending_review_comment_count();
    let title = review
        .selected_diff_file()
        .map(|file| {
            if pending_count > 0 {
                format!(" Diff: {}  [pending: {}] ", file.path, pending_count)
            } else {
                format!(" Diff: {} ", file.path)
            }
        })
        .unwrap_or_else(|| " Diff ".to_owned());
    let block = Block::default()
        .title(Span::styled(title, theme::title()))
        .borders(Borders::ALL)
        .border_style(border_style);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    review.sync_pending_review_preview_target();
    let hovered_pending_comment = review.selected_pending_review_comment().cloned();
    let (diff_area, pending_preview_area) = if hovered_pending_comment.is_some() && inner.height > 3
    {
        let sections =
            Layout::vertical([Constraint::Percentage(80), Constraint::Percentage(20)]).split(inner);
        (sections[0], Some(sections[1]))
    } else {
        (inner, None)
    };

    let (text_area, scrollbar_area) = if diff_area.width > 1 {
        let sections =
            Layout::horizontal([Constraint::Min(1), Constraint::Length(1)]).split(diff_area);
        (sections[0], Some(sections[1]))
    } else {
        (diff_area, None)
    };
    review.set_diff_viewport_height(text_area.height.max(1));

    let viewport_height = usize::from(text_area.height.max(1));
    let (lines, content_height, scroll) = if let Some(file) = review.selected_diff_file() {
        let (left, right) = markdown.diff_file_highlights(file);
        let content_height = file.rows.len().max(1);
        let max_scroll = content_height.saturating_sub(viewport_height);
        let scroll = usize::from(review.diff_scroll).min(max_scroll);
        let pending = review
            .pending_review_comments_for_file(file)
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let lines = review_diff::render_rows(DiffRowsRenderContext {
            file,
            width: text_area.width,
            left_syntax: left,
            right_syntax: right,
            row_offset: scroll,
            row_limit: viewport_height,
            selected_line: review.selected_diff_line(),
            selected_range: review.selected_diff_range(),
            pending_comments: &pending,
        });
        (lines, content_height, scroll)
    } else if let Some(error) = &review.diff_error {
        let lines = vec![Line::from(vec![Span::styled(
            format!("Diff unavailable: {error}"),
            theme::error(),
        )])];
        (lines, 1usize, 0usize)
    } else {
        let lines = vec![Line::from(vec![Span::styled(
            "Loading pull request diff...",
            theme::dim(),
        )])];
        (lines, 1usize, 0usize)
    };

    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, text_area);

    if content_height > viewport_height
        && let Some(area) = scrollbar_area
    {
        let max_scroll = content_height.saturating_sub(viewport_height);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None)
            .track_style(theme::dim())
            .thumb_style(theme::title());
        let mut scrollbar_state = ScrollbarState::new(max_scroll.saturating_add(1))
            .viewport_content_length(viewport_height)
            .position(scroll);
        frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
    }

    if let (Some(preview_area), Some(comment)) = (pending_preview_area, hovered_pending_comment) {
        render_pending_comment_preview(frame, preview_area, review, markdown, &comment);
    }
}

fn render_pending_comment_preview(
    frame: &mut Frame<'_>,
    area: Rect,
    review: &mut ReviewScreenState,
    markdown: &mut MarkdownRenderer,
    comment: &PendingReviewCommentDraft,
) {
    let location = if let Some(start) = comment.start_line {
        format!("{}:{}-{}", comment.path, start, comment.line)
    } else {
        format!("{}:{}", comment.path, comment.line)
    };
    let title = format!(" Pending Comment Preview ({location}) ");
    let block = Block::default()
        .title(Span::styled(title, theme::title()))
        .borders(Borders::ALL)
        .border_style(theme::border());
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let (text_area, scrollbar_area) = if inner.width > 1 {
        let columns = Layout::horizontal([Constraint::Min(1), Constraint::Length(1)]).split(inner);
        (columns[0], Some(columns[1]))
    } else {
        (inner, None)
    };

    let mut lines = markdown.render(&comment.body);
    if lines.is_empty() {
        lines.push(Line::from(vec![Span::styled("(empty)", theme::dim())]));
    }

    let viewport_height = usize::from(text_area.height.max(1));
    let content_height = wrapped_content_height(&lines, text_area.width.max(1));
    let max_scroll = content_height.saturating_sub(viewport_height);
    let scroll = review.clamp_pending_preview_scroll(max_scroll);
    let paragraph = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((u16::try_from(scroll).unwrap_or(u16::MAX), 0));
    frame.render_widget(paragraph, text_area);

    if content_height > viewport_height
        && let Some(scrollbar_area) = scrollbar_area
    {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None)
            .track_style(theme::dim())
            .thumb_style(theme::title());
        let mut scrollbar_state = ScrollbarState::new(max_scroll.saturating_add(1))
            .viewport_content_length(viewport_height)
            .position(scroll);
        frame.render_stateful_widget(scrollbar, scrollbar_area, &mut scrollbar_state);
    }
}

fn wrapped_content_height(lines: &[Line<'_>], width: u16) -> usize {
    if width == 0 {
        return 0;
    }

    let width = usize::from(width);
    lines
        .iter()
        .map(|line| {
            let line_width = line.width();
            if line_width == 0 {
                1
            } else {
                (line_width - 1) / width + 1
            }
        })
        .sum()
}
