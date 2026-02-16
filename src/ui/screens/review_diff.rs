//! Diff row renderer shared by the review diff pane.

use crate::app::state::{PendingReviewCommentDraft, PendingReviewCommentSide};
use crate::domain::{
    PullRequestDiffFile, PullRequestDiffHighlightRange, PullRequestDiffRow, PullRequestDiffRowKind,
};
use crate::ui::theme;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};

pub(crate) struct DiffRowsRenderContext<'a> {
    pub file: &'a PullRequestDiffFile,
    pub width: u16,
    pub left_syntax: &'a [Vec<Option<Color>>],
    pub right_syntax: &'a [Vec<Option<Color>>],
    pub row_offset: usize,
    pub row_limit: usize,
    pub selected_line: usize,
    pub selected_range: Option<(usize, usize)>,
    pub pending_comments: &'a [PendingReviewCommentDraft],
}

pub(crate) fn render_rows(context: DiffRowsRenderContext<'_>) -> Vec<Line<'static>> {
    let DiffRowsRenderContext {
        file,
        width,
        left_syntax,
        right_syntax,
        row_offset,
        row_limit,
        selected_line,
        selected_range,
        pending_comments,
    } = context;

    let width = usize::from(width.max(1));
    let marker_width = 2usize;
    let separator = " │ ";
    let available = width.saturating_sub(marker_width + separator.len());
    let left_width = available / 2;
    let right_width = available.saturating_sub(left_width);

    file.rows
        .iter()
        .enumerate()
        .skip(row_offset)
        .take(row_limit)
        .map(|(row_index, row)| {
            let in_selected_range =
                selected_range.is_some_and(|(start, end)| row_index >= start && row_index <= end);
            let has_pending = pending_comments
                .iter()
                .any(|comment| pending_comment_matches_row(comment, row));
            let (marker, marker_style) = if row_index == selected_line {
                ("▌ ", theme::open_thread())
            } else if has_pending {
                ("● ", theme::resolved_thread())
            } else if in_selected_range {
                ("│ ", theme::dim())
            } else {
                ("  ", theme::dim())
            };

            let mut spans = Vec::new();
            spans.push(Span::styled(marker.to_owned(), marker_style));
            spans.extend(render_diff_side(DiffSideRenderContext {
                line_number: row.left_line_number,
                text: &row.left_text,
                width: left_width,
                syntax_fg: row
                    .left_line_number
                    .and_then(|line| left_syntax.get(line.saturating_sub(1)).map(Vec::as_slice)),
                highlights: &row.left_highlights,
                row_kind: row.kind,
                side: DiffSide::Left,
                in_selected_range,
            }));
            spans.push(Span::styled(separator.to_owned(), theme::dim()));
            spans.extend(render_diff_side(DiffSideRenderContext {
                line_number: row.right_line_number,
                text: &row.right_text,
                width: right_width,
                syntax_fg: row
                    .right_line_number
                    .and_then(|line| right_syntax.get(line.saturating_sub(1)).map(Vec::as_slice)),
                highlights: &row.right_highlights,
                row_kind: row.kind,
                side: DiffSide::Right,
                in_selected_range,
            }));
            Line::from(spans)
        })
        .collect()
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum DiffSide {
    Left,
    Right,
}

struct DiffSideRenderContext<'a> {
    line_number: Option<usize>,
    text: &'a str,
    width: usize,
    syntax_fg: Option<&'a [Option<Color>]>,
    highlights: &'a [PullRequestDiffHighlightRange],
    row_kind: PullRequestDiffRowKind,
    side: DiffSide,
    in_selected_range: bool,
}

fn render_diff_side(context: DiffSideRenderContext<'_>) -> Vec<Span<'static>> {
    let DiffSideRenderContext {
        line_number,
        text,
        width,
        syntax_fg,
        highlights,
        row_kind,
        side,
        in_selected_range,
    } = context;
    if width == 0 {
        return Vec::new();
    }

    let number_width = width.min(6);
    let text_width = width.saturating_sub(number_width);
    let number =
        line_number.map_or_else(|| " ".repeat(number_width), |value| format!("{value:>5} "));

    let visible_chars = text.chars().take(text_width).collect::<Vec<_>>();
    let visible_len = visible_chars.len();
    let color_set = DiffRowColors::new(row_kind, side);
    let alignment_gap = text.trim().is_empty()
        && line_number.is_none()
        && row_kind != PullRequestDiffRowKind::Context;
    let number_bg = if alignment_gap {
        Color::Reset
    } else {
        color_set.content_bg.unwrap_or(Color::Reset)
    };

    let number_style = Style::default()
        .fg(theme::dim().fg.unwrap_or(Color::DarkGray))
        .bg(number_bg);
    let content_style = Style::default()
        .fg(color_set.content_fg)
        .bg(color_set.content_bg.unwrap_or(Color::Reset));
    let highlight_style = Style::default()
        .fg(color_set.highlight_fg)
        .bg(color_set.highlight_bg.unwrap_or(Color::Reset));
    let selection_bg =
        theme::blend_with_terminal_bg(theme::open_thread().fg.unwrap_or(Color::Cyan), 0.22);
    let with_selection = |style: Style| {
        if in_selected_range {
            style.bg(selection_bg)
        } else {
            style
        }
    };
    let number_style = with_selection(number_style);
    let content_style = with_selection(content_style);
    let highlight_style = with_selection(highlight_style);

    let mut spans = vec![Span::styled(number, number_style)];
    let line_highlights = clip_and_merge_ranges(highlights, visible_len);
    let should_use_partial = row_kind == PullRequestDiffRowKind::Modified
        && !line_highlights.is_empty()
        && !ranges_cover_visible_content(&line_highlights, visible_len);

    if text_width == 0 {
        return spans;
    }

    if text.trim().is_empty() {
        let filler_style = if alignment_gap {
            Style::default().fg(theme::diff_context().fg.unwrap_or(color_set.dim_fg))
        } else if row_kind == PullRequestDiffRowKind::Modified {
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
            syntax_fg,
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

    let fill_style = if row_kind == PullRequestDiffRowKind::Modified {
        highlight_style
    } else {
        content_style
    };
    spans.extend(render_syntax_cells(
        &visible_chars,
        syntax_fg,
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

struct DiffRowColors {
    content_fg: Color,
    content_bg: Option<Color>,
    highlight_fg: Color,
    highlight_bg: Option<Color>,
    dim_fg: Color,
}

impl DiffRowColors {
    fn new(row_kind: PullRequestDiffRowKind, side: DiffSide) -> Self {
        let base_fg = theme::text().fg.unwrap_or(Color::White);
        let dim_fg = theme::dim().fg.unwrap_or(Color::DarkGray);
        let add_fg = theme::diff_add().fg.unwrap_or(Color::Green);
        let remove_fg = theme::diff_remove().fg.unwrap_or(Color::Red);
        let modified_left_fg = theme::issue().fg.unwrap_or(Color::Yellow);
        let modified_right_fg = add_fg;
        let add_bg = theme::blend_with_terminal_bg(add_fg, 0.22);
        let remove_bg = theme::blend_with_terminal_bg(remove_fg, 0.22);
        let modified_left_bg = theme::blend_with_terminal_bg(modified_left_fg, 0.16);
        let modified_right_bg = theme::blend_with_terminal_bg(modified_right_fg, 0.16);
        let dim_bg = theme::blend_with_terminal_bg(dim_fg, 0.08);
        let (content_fg, content_bg, highlight_fg, highlight_bg) = match row_kind {
            PullRequestDiffRowKind::Context => (base_fg, None, base_fg, None),
            PullRequestDiffRowKind::Added => {
                if side == DiffSide::Left {
                    (dim_fg, Some(dim_bg), dim_fg, Some(dim_bg))
                } else {
                    (add_fg, Some(add_bg), add_fg, Some(add_bg))
                }
            }
            PullRequestDiffRowKind::Removed => {
                if side == DiffSide::Left {
                    (remove_fg, Some(remove_bg), remove_fg, Some(remove_bg))
                } else {
                    (dim_fg, Some(dim_bg), dim_fg, Some(dim_bg))
                }
            }
            PullRequestDiffRowKind::Modified => {
                if side == DiffSide::Left {
                    (
                        base_fg,
                        Some(modified_left_bg),
                        modified_left_fg,
                        Some(modified_left_bg),
                    )
                } else {
                    (
                        base_fg,
                        Some(modified_right_bg),
                        modified_right_fg,
                        Some(modified_right_bg),
                    )
                }
            }
        };

        Self {
            content_fg,
            content_bg,
            highlight_fg,
            highlight_bg,
            dim_fg,
        }
    }
}

fn pending_comment_matches_row(
    comment: &PendingReviewCommentDraft,
    row: &PullRequestDiffRow,
) -> bool {
    let Some(line) = row_line_for_pending_side(row, comment.side) else {
        return false;
    };
    let line = line as u64;
    let start = comment.start_line.unwrap_or(comment.line).min(comment.line);
    let end = comment.start_line.unwrap_or(comment.line).max(comment.line);
    line >= start && line <= end
}

fn row_line_for_pending_side(
    row: &PullRequestDiffRow,
    side: PendingReviewCommentSide,
) -> Option<usize> {
    match side {
        PendingReviewCommentSide::Left => row.left_line_number,
        PendingReviewCommentSide::Right => row.right_line_number,
    }
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
    "╱".repeat(width)
}
