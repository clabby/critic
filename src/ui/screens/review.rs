//! Review screen renderer with tabs for threads and diffs.

use crate::app::state::{AppState, ReviewScreenState, ReviewTab};
use crate::domain::{
    CommentRef, ListNodeKind, PullRequestDiffFileStatus, PullRequestDiffHighlightRange,
    PullRequestDiffRowKind,
};
use crate::render::markdown::MarkdownRenderer;
use crate::render::thread::{
    render_issue_preview, render_review_summary_preview, render_thread_preview,
};
use crate::ui::components::shared::short_preview;
use crate::ui::theme;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, List, ListItem, ListState, Paragraph, Scrollbar, ScrollbarOrientation,
    ScrollbarState, Tabs, Wrap,
};

pub fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    state: &AppState,
    markdown: &mut MarkdownRenderer,
) {
    let Some(review) = state.review.as_ref() else {
        frame.render_widget(
            Paragraph::new("No pull request selected.").block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(theme::border()),
            ),
            area,
        );
        return;
    };

    let rows = Layout::vertical([Constraint::Length(3), Constraint::Min(1)]).split(area);
    render_tabs(frame, rows[0], review.active_tab());

    match review.active_tab() {
        ReviewTab::Threads => render_threads_tab(frame, rows[1], review, markdown),
        ReviewTab::Diff => render_diff_tab(frame, rows[1], review, markdown),
    }
}

fn render_tabs(frame: &mut Frame<'_>, area: Rect, tab: ReviewTab) {
    let titles = vec![" Threads ", " Diff "];
    let selected = match tab {
        ReviewTab::Threads => 0,
        ReviewTab::Diff => 1,
    };

    let tabs = Tabs::new(titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme::border()),
        )
        .highlight_style(theme::selected())
        .style(theme::dim())
        .select(selected);

    frame.render_widget(tabs, area);
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
    review: &ReviewScreenState,
    markdown: &mut MarkdownRenderer,
) {
    let panes =
        Layout::horizontal([Constraint::Percentage(25), Constraint::Percentage(75)]).split(area);

    render_diff_files(frame, panes[0], review);
    render_diff_content(frame, panes[1], review, markdown);
}

fn render_left_pane(frame: &mut Frame<'_>, area: Rect, review: &ReviewScreenState) {
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

    frame.render_stateful_widget(list, area, &mut list_state);
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

    let mut lines = if let Some(node) = review.selected_node() {
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

    fill_fenced_code_backgrounds(&mut lines, text_area.width);

    let viewport_height = usize::from(text_area.height);
    let content_height = wrapped_content_height(&lines, text_area.width);
    let max_scroll = content_height.saturating_sub(viewport_height);
    let scroll = usize::from(review.right_scroll).min(max_scroll);

    let paragraph = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((u16::try_from(scroll).unwrap_or(u16::MAX), 0));

    frame.render_widget(paragraph, text_area);

    if content_height > viewport_height {
        if let Some(area) = scrollbar_area {
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
}

fn render_diff_files(frame: &mut Frame<'_>, area: Rect, review: &ReviewScreenState) {
    let file_count = review.diff_file_count();
    let rows = review.diff_tree_rows();
    let row_count = rows.len();
    let block = Block::default()
        .title(Span::styled(
            format!(" Files ({file_count}) ",),
            theme::title(),
        ))
        .borders(Borders::ALL)
        .border_style(theme::border());
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let sections = Layout::vertical([Constraint::Length(3), Constraint::Min(0)]).split(inner);
    let search_area = sections[0];
    let body_area = sections[1];

    let search_line = if review.diff_search_query().is_empty() {
        Line::from(vec![
            Span::styled(" Filter: ", theme::dim()),
            Span::styled("(press [s] to search files)", theme::dim()),
        ])
    } else {
        Line::from(vec![
            Span::styled(" Filter: ", theme::dim()),
            Span::styled(review.diff_search_query().to_owned(), theme::text()),
        ])
    };
    let search_style = if review.is_diff_search_focused() {
        theme::selected()
    } else {
        Style::default()
    };
    let search_widget = Paragraph::new(search_line).style(search_style).block(
        Block::default()
            .borders(Borders::BOTTOM)
            .border_style(theme::border()),
    );
    frame.render_widget(search_widget, search_area);

    let (list_area, scrollbar_area) = if body_area.width > 1 {
        let columns =
            Layout::horizontal([Constraint::Min(1), Constraint::Length(1)]).split(body_area);
        (columns[0], Some(columns[1]))
    } else {
        (body_area, None)
    };

    let items: Vec<ListItem<'static>> = if let Some(diff) = &review.diff {
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
                        Span::styled(format!("{indent}"), theme::dim()),
                        status,
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

    if list_area.height > 0 && row_count > usize::from(list_area.height) {
        if let Some(scrollbar_area) = scrollbar_area {
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
}

fn render_diff_content(
    frame: &mut Frame<'_>,
    area: Rect,
    review: &ReviewScreenState,
    markdown: &mut MarkdownRenderer,
) {
    let title = review
        .selected_diff_file()
        .map(|file| {
            format!(
                " Diff: {} [{}] ",
                file.path,
                markdown.current_syntax_theme_name()
            )
        })
        .unwrap_or_else(|| " Diff ".to_owned());
    let block = Block::default()
        .title(Span::styled(title, theme::title()))
        .borders(Borders::ALL)
        .border_style(theme::border());
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let (text_area, scrollbar_area) = if inner.width > 1 {
        let sections = Layout::horizontal([Constraint::Min(1), Constraint::Length(1)]).split(inner);
        (sections[0], Some(sections[1]))
    } else {
        (inner, None)
    };

    let lines = if let Some(file) = review.selected_diff_file() {
        let (left, right) = markdown.diff_file_highlights(file);
        render_diff_rows(file, text_area.width, left, right)
    } else if let Some(error) = &review.diff_error {
        vec![Line::from(vec![Span::styled(
            format!("Diff unavailable: {error}"),
            theme::error(),
        )])]
    } else {
        vec![Line::from(vec![Span::styled(
            "Loading pull request diff...",
            theme::dim(),
        )])]
    };

    let viewport_height = usize::from(text_area.height.max(1));
    let content_height = lines.len().max(1);
    let max_scroll = content_height.saturating_sub(viewport_height);
    let scroll = usize::from(review.diff_scroll).min(max_scroll);

    let paragraph = Paragraph::new(lines).scroll((u16::try_from(scroll).unwrap_or(u16::MAX), 0));
    frame.render_widget(paragraph, text_area);

    if content_height > viewport_height {
        if let Some(area) = scrollbar_area {
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
    }
}

fn render_diff_rows(
    file: &crate::domain::PullRequestDiffFile,
    width: u16,
    left_syntax: &[Vec<Option<Color>>],
    right_syntax: &[Vec<Option<Color>>],
) -> Vec<Line<'static>> {
    let width = usize::from(width.max(1));
    let separator = " │ ";
    let available = width.saturating_sub(separator.len());
    let left_width = available / 2;
    let right_width = available.saturating_sub(left_width);

    file.rows
        .iter()
        .map(|row| {
            let mut spans = Vec::new();
            spans.extend(render_diff_side(
                row.left_line_number,
                &row.left_text,
                left_width,
                row.left_line_number
                    .and_then(|line| left_syntax.get(line.saturating_sub(1)).map(Vec::as_slice)),
                &row.left_highlights,
                row.kind,
                true,
            ));
            spans.push(Span::styled(separator.to_owned(), theme::dim()));
            spans.extend(render_diff_side(
                row.right_line_number,
                &row.right_text,
                right_width,
                row.right_line_number
                    .and_then(|line| right_syntax.get(line.saturating_sub(1)).map(Vec::as_slice)),
                &row.right_highlights,
                row.kind,
                false,
            ));
            Line::from(spans)
        })
        .collect()
}

fn render_diff_side(
    line_number: Option<usize>,
    text: &str,
    width: usize,
    syntax_fg: Option<&[Option<Color>]>,
    highlights: &[PullRequestDiffHighlightRange],
    kind: PullRequestDiffRowKind,
    is_left: bool,
) -> Vec<Span<'static>> {
    if width == 0 {
        return Vec::new();
    }

    let number_width = width.min(6);
    let text_width = width.saturating_sub(number_width);
    let number = line_number
        .map(|value| format!("{value:>5} "))
        .unwrap_or_else(|| " ".repeat(number_width));

    let visible_chars = text.chars().take(text_width).collect::<Vec<_>>();
    let visible_len = visible_chars.len();
    let syntax_fg = syntax_fg.map(|line| {
        line.iter()
            .copied()
            .take(visible_len)
            .chain(std::iter::repeat(None))
            .take(visible_len)
            .collect::<Vec<_>>()
    });
    let base_fg = theme::text().fg.unwrap_or(Color::White);
    let (content_fg, content_bg, highlight_fg, highlight_bg) = match kind {
        PullRequestDiffRowKind::Context => (base_fg, None, base_fg, None),
        PullRequestDiffRowKind::Added => {
            if is_left {
                (
                    theme::dim().fg.unwrap_or(Color::DarkGray),
                    Some(Color::Rgb(24, 24, 24)),
                    theme::dim().fg.unwrap_or(Color::DarkGray),
                    Some(Color::Rgb(24, 24, 24)),
                )
            } else {
                (
                    Color::Rgb(172, 218, 170),
                    Some(Color::Rgb(23, 42, 26)),
                    Color::Rgb(172, 218, 170),
                    Some(Color::Rgb(23, 42, 26)),
                )
            }
        }
        PullRequestDiffRowKind::Removed => {
            if is_left {
                (
                    Color::Rgb(229, 161, 161),
                    Some(Color::Rgb(52, 23, 25)),
                    Color::Rgb(229, 161, 161),
                    Some(Color::Rgb(52, 23, 25)),
                )
            } else {
                (
                    theme::dim().fg.unwrap_or(Color::DarkGray),
                    Some(Color::Rgb(24, 24, 24)),
                    theme::dim().fg.unwrap_or(Color::DarkGray),
                    Some(Color::Rgb(24, 24, 24)),
                )
            }
        }
        PullRequestDiffRowKind::Modified => {
            if is_left {
                (
                    base_fg,
                    Some(Color::Rgb(24, 24, 24)),
                    Color::Rgb(236, 193, 170),
                    Some(Color::Rgb(58, 43, 35)),
                )
            } else {
                (
                    base_fg,
                    Some(Color::Rgb(24, 24, 24)),
                    Color::Rgb(194, 220, 194),
                    Some(Color::Rgb(34, 48, 34)),
                )
            }
        }
    };

    let number_style = Style::default()
        .fg(theme::dim().fg.unwrap_or(Color::DarkGray))
        .bg(content_bg.unwrap_or(Color::Reset));
    let content_style = Style::default()
        .fg(content_fg)
        .bg(content_bg.unwrap_or(Color::Reset));
    let highlight_style = Style::default()
        .fg(highlight_fg)
        .bg(highlight_bg.unwrap_or(Color::Reset));

    let mut spans = vec![Span::styled(number, number_style)];
    let line_highlights = clip_and_merge_ranges(highlights, visible_len);
    let should_use_partial = kind == PullRequestDiffRowKind::Modified
        && !line_highlights.is_empty()
        && !ranges_cover_visible_content(&line_highlights, visible_len);

    if text_width == 0 {
        return spans;
    }

    if text.trim().is_empty() {
        let alignment_gap = line_number.is_none() && kind != PullRequestDiffRowKind::Context;
        let filler_style = if alignment_gap {
            Style::default()
                .fg(Color::Rgb(57, 72, 114))
                .bg(content_bg.unwrap_or(Color::Reset))
        } else if kind == PullRequestDiffRowKind::Modified {
            highlight_style
        } else {
            content_style
        };
        let filler = if alignment_gap {
            hatched_filler(text_width)
        } else {
            " ".repeat(text_width)
        };
        spans.push(Span::styled(filler, filler_style));
        return spans;
    }

    if should_use_partial {
        let highlight_mask = mask_highlight_ranges(&line_highlights, visible_len);
        spans.extend(render_syntax_cells(
            &visible_chars,
            syntax_fg.as_deref(),
            content_style,
            highlight_style,
            Some(&highlight_mask),
        ));
        if visible_len < text_width {
            spans.push(Span::styled(
                " ".repeat(text_width - visible_len),
                content_style,
            ));
        }
        return spans;
    }

    let fill_style = if kind == PullRequestDiffRowKind::Modified {
        highlight_style
    } else {
        content_style
    };
    spans.extend(render_syntax_cells(
        &visible_chars,
        syntax_fg.as_deref(),
        fill_style,
        fill_style,
        None,
    ));
    if visible_len < text_width {
        spans.push(Span::styled(
            " ".repeat(text_width - visible_len),
            fill_style,
        ));
    }
    spans
}

fn clip_and_merge_ranges(
    ranges: &[PullRequestDiffHighlightRange],
    visible_len: usize,
) -> Vec<PullRequestDiffHighlightRange> {
    if ranges.is_empty() || visible_len == 0 {
        return Vec::new();
    }

    let mut clipped = ranges
        .iter()
        .filter_map(|range| {
            let start = range.start.min(visible_len);
            let end = range.end.min(visible_len);
            (start < end).then_some(PullRequestDiffHighlightRange { start, end })
        })
        .collect::<Vec<_>>();

    if clipped.is_empty() {
        return clipped;
    }

    clipped.sort_unstable_by_key(|range| (range.start, range.end));
    let mut merged: Vec<PullRequestDiffHighlightRange> = Vec::with_capacity(clipped.len());
    for range in clipped {
        match merged.last_mut() {
            Some(last) if range.start <= last.end => {
                last.end = last.end.max(range.end);
            }
            _ => merged.push(range),
        }
    }

    merged
}

fn ranges_cover_visible_content(
    ranges: &[PullRequestDiffHighlightRange],
    visible_len: usize,
) -> bool {
    if visible_len == 0 {
        return false;
    }

    let mut covered_until = 0usize;
    for range in ranges {
        if range.start > covered_until {
            return false;
        }
        covered_until = covered_until.max(range.end);
        if covered_until >= visible_len {
            return true;
        }
    }

    false
}

fn mask_highlight_ranges(ranges: &[PullRequestDiffHighlightRange], width: usize) -> Vec<bool> {
    let mut mask = vec![false; width];
    for range in ranges {
        let start = range.start.min(width);
        let end = range.end.min(width);
        for flag in mask.iter_mut().take(end).skip(start) {
            *flag = true;
        }
    }
    mask
}

fn render_syntax_cells(
    chars: &[char],
    syntax_fg: Option<&[Option<Color>]>,
    base_style: Style,
    highlight_style: Style,
    highlight_mask: Option<&[bool]>,
) -> Vec<Span<'static>> {
    if chars.is_empty() {
        return Vec::new();
    }

    let mut spans = Vec::new();
    let mut buffer = String::new();
    let mut current_style: Option<Style> = None;

    for (index, ch) in chars.iter().enumerate() {
        let highlighted = highlight_mask
            .and_then(|mask| mask.get(index))
            .copied()
            .unwrap_or(false);

        let mut style = if highlighted {
            highlight_style
        } else {
            base_style
        };
        if let Some(color) = syntax_fg.and_then(|fg| fg.get(index)).copied().flatten() {
            style = style.fg(color);
        }

        if current_style == Some(style) {
            buffer.push(*ch);
            continue;
        }

        if !buffer.is_empty() {
            spans.push(Span::styled(
                std::mem::take(&mut buffer),
                current_style.unwrap_or(base_style),
            ));
        }

        current_style = Some(style);
        buffer.push(*ch);
    }

    if !buffer.is_empty() {
        spans.push(Span::styled(buffer, current_style.unwrap_or(base_style)));
    }

    spans
}

fn hatched_filler(width: usize) -> String {
    let mut filler = String::with_capacity(width);
    for index in 0..width {
        if index % 2 == 0 {
            filler.push('╱');
        } else {
            filler.push('╲');
        }
    }
    filler
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

fn fill_fenced_code_backgrounds(lines: &mut [Line<'static>], width: u16) {
    let width = usize::from(width);
    if width == 0 {
        return;
    }

    for line in lines {
        if !is_fenced_code_line(line) {
            continue;
        }

        let Some(background) = line.spans.iter().find_map(|span| span.style.bg) else {
            continue;
        };

        let line_width = line.width();
        if line_width >= width {
            continue;
        }

        let padding = width - line_width;
        line.spans.push(Span::styled(
            " ".repeat(padding),
            theme::code_padding().bg(background),
        ));
    }
}

fn is_fenced_code_line(line: &Line<'_>) -> bool {
    let has_background = line.spans.iter().any(|span| span.style.bg.is_some());
    if !has_background {
        return false;
    }

    let leading_space_spans = line
        .spans
        .iter()
        .take_while(|span| span.content.chars().all(|ch| ch == ' '))
        .count();

    leading_space_spans >= 2
}
