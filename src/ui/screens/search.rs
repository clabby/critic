//! Pull request fuzzy-search screen renderer.

use crate::{
    app::state::{AppState, SearchSort},
    domain::PullRequestReviewStatus,
    ui::{
        components::{
            search_box,
            shared::{short_preview, short_timestamp},
        },
        theme,
    },
};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{
        Block, Borders, Cell, HighlightSpacing, Paragraph, Row, Scrollbar, ScrollbarOrientation,
        ScrollbarState, Table, TableState,
    },
};

const AGE_COL_WIDTH: u16 = 4;
const STATUS_COL_WIDTH: u16 = 1;
const MIN_AUTHOR_COL_WIDTH: u16 = 8;
const MAX_AUTHOR_COL_WIDTH: u16 = 16;
const MIN_TITLE_COL_WIDTH: u16 = 16;
const COLUMN_SPACING: u16 = 1;

const CONTROL_BOX_WIDTH: u16 = 18;

pub fn render(frame: &mut Frame<'_>, area: Rect, state: &AppState) {
    let rows = Layout::vertical([Constraint::Length(3), Constraint::Min(6)]).split(area);
    let controls = Layout::horizontal([
        Constraint::Min(24),
        Constraint::Length(CONTROL_BOX_WIDTH),
        Constraint::Length(CONTROL_BOX_WIDTH),
        Constraint::Length(CONTROL_BOX_WIDTH),
    ])
    .split(rows[0]);

    render_search_box(frame, controls[0], state);
    render_scope_box(frame, controls[1], state);
    render_status_box(frame, controls[2], state);
    render_sort_box(frame, controls[3], state);
    render_results(frame, rows[1], state);
}

fn render_search_box(frame: &mut Frame<'_>, area: Rect, state: &AppState) {
    let title = if state.is_search_focused() {
        Line::from(vec![Span::styled(" PR Search ", theme::info())])
    } else {
        Line::from(vec![
            Span::styled(" PR Search ", theme::title()),
            Span::styled("[s] ", theme::info()),
        ])
    };

    search_box::render(
        frame,
        area,
        search_box::SearchBoxProps {
            title: " PR Search ",
            title_line: Some(title),
            query: state.search_query(),
            focused: state.is_search_focused(),
            focused_placeholder: "",
            unfocused_placeholder: "search...",
            focused_right_hint: Some("[⏎/␛]"),
        },
    );
}

fn render_scope_box(frame: &mut Frame<'_>, area: Rect, state: &AppState) {
    let title = if state.is_search_focused() {
        Line::from(vec![Span::styled(" Scope ", theme::title())])
    } else {
        Line::from(vec![
            Span::styled(" Scope ", theme::title()),
            Span::styled("[u] ", theme::info()),
        ])
    };
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(theme::border());

    let value = state.search_scope.label();
    let line = Line::from(vec![Span::raw("  "), Span::styled(value, theme::text())]);

    frame.render_widget(Paragraph::new(line).block(block), area);
}

fn render_status_box(frame: &mut Frame<'_>, area: Rect, state: &AppState) {
    let title = if state.is_search_focused() {
        Line::from(vec![Span::styled(" Status ", theme::title())])
    } else {
        Line::from(vec![
            Span::styled(" Status ", theme::title()),
            Span::styled("[i] ", theme::info()),
        ])
    };
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(theme::border());

    let value = state.search_status_filter.label();
    let line = Line::from(vec![Span::raw("  "), Span::styled(value, theme::text())]);

    frame.render_widget(Paragraph::new(line).block(block), area);
}

fn render_sort_box(frame: &mut Frame<'_>, area: Rect, state: &AppState) {
    let title = if state.is_search_focused() {
        Line::from(vec![Span::styled(" Sort ", theme::title())])
    } else {
        Line::from(vec![
            Span::styled(" Sort ", theme::title()),
            Span::styled("[o] ", theme::info()),
        ])
    };
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(theme::border());

    let value = state.search_sort.label();
    let line = Line::from(vec![Span::raw("  "), Span::styled(value, theme::text())]);

    frame.render_widget(Paragraph::new(line).block(block), area);
}

fn render_results(frame: &mut Frame<'_>, area: Rect, state: &AppState) {
    let title = if state.is_search_focused() {
        Line::from(vec![
            Span::styled(" Open Pull Requests ", theme::title()),
            Span::styled(format!("({}) ", state.search_results.len()), theme::dim()),
        ])
    } else {
        Line::from(vec![
            Span::styled(" Open Pull Requests ", theme::title()),
            Span::styled(format!("({})", state.search_results.len()), theme::dim()),
            Span::raw(" "),
            Span::styled("[j/k]", theme::info()),
            Span::styled(" move  ", theme::dim()),
            Span::styled("[⏎]", theme::info()),
            Span::styled(" open  ", theme::dim()),
            Span::styled("[W]", theme::info()),
            Span::styled(" web  ", theme::dim()),
            Span::styled("[R]", theme::info()),
            Span::styled(" refresh ", theme::dim()),
        ])
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(theme::border());
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let (list_area, scrollbar_area) = if inner.width > 1 {
        let columns = Layout::horizontal([Constraint::Min(1), Constraint::Length(1)]).split(inner);
        (columns[0], Some(columns[1]))
    } else {
        (inner, None)
    };

    if state.search_results.is_empty() {
        let msg = Paragraph::new(Line::styled(
            "No open pull requests match this query.",
            theme::dim(),
        ));
        frame.render_widget(msg, list_area);
        return;
    }

    let number_col_width = state
        .search_results
        .iter()
        .filter_map(|index| state.pull_requests.get(*index))
        .map(|pull| format!("#{}", pull.number).len() as u16)
        .max()
        .unwrap_or(2);

    // The highlight symbol "▸ " occupies 2 columns; 4 inter-column gaps each cost COLUMN_SPACING.
    let fixed = AGE_COL_WIDTH + STATUS_COL_WIDTH + number_col_width;
    let overhead = COLUMN_SPACING * 4 + 2;
    let available_for_author = list_area
        .width
        .saturating_sub(fixed + overhead + MIN_TITLE_COL_WIDTH);
    let author_col_width = if available_for_author >= MIN_AUTHOR_COL_WIDTH {
        available_for_author.min(MAX_AUTHOR_COL_WIDTH)
    } else {
        available_for_author
    };

    let widths = [
        Constraint::Length(AGE_COL_WIDTH),
        Constraint::Length(STATUS_COL_WIDTH),
        Constraint::Length(number_col_width),
        Constraint::Fill(1),
        Constraint::Length(author_col_width),
    ];

    let rows: Vec<Row<'_>> = state
        .search_results
        .iter()
        .filter_map(|index| state.pull_requests.get(*index))
        .map(|pull| {
            let (status_text, status_style) = if pull.is_draft {
                ("D", theme::dim())
            } else {
                match pull.review_status {
                    Some(PullRequestReviewStatus::Approved) => ("A", theme::resolved_thread()),
                    Some(PullRequestReviewStatus::ChangesRequested) => ("R", theme::error()),
                    None => ("", theme::dim()),
                }
            };

            let age_ms = match state.search_sort {
                SearchSort::UpdatedAt => pull.updated_at_unix_ms,
                SearchSort::CreatedAt => pull.created_at_unix_ms,
            };

            let (author_marker, author_marker_style) = if state
                .viewer_login
                .as_deref()
                .is_some_and(|login| pull.author.eq_ignore_ascii_case(login))
            {
                ("@ ", theme::dim())
            } else if state
                .viewer_login
                .as_deref()
                .is_some_and(|login| pull.has_reviewer(login))
            {
                ("R ", theme::dim())
            } else {
                ("  ", theme::dim())
            };

            let author_text_width = author_col_width.saturating_sub(2) as usize;
            let author_text = if author_text_width == 0 {
                String::new()
            } else {
                short_preview(&pull.author, author_text_width)
            };

            Row::new([
                Cell::new(Span::styled(short_timestamp(age_ms), theme::dim())),
                Cell::new(Span::styled(status_text, status_style)),
                Cell::new(
                    Line::styled(format!("#{}", pull.number), theme::title())
                        .alignment(Alignment::Right),
                ),
                Cell::new(pull.title.clone()),
                Cell::new(Line::from(vec![
                    Span::styled(author_marker, author_marker_style),
                    Span::styled(author_text, theme::dim()),
                ])),
            ])
        })
        .collect();

    let table = Table::new(rows, widths)
        .column_spacing(COLUMN_SPACING)
        .row_highlight_style(theme::selected())
        .highlight_symbol("▸ ")
        .highlight_spacing(HighlightSpacing::Always);

    let mut table_state = TableState::default();
    table_state.select(Some(state.search_selected));

    frame.render_stateful_widget(table, list_area, &mut table_state);

    let viewport_height = usize::from(list_area.height);
    let content_height = state.search_results.len();

    if content_height > viewport_height
        && let Some(scrollbar_area) = scrollbar_area
    {
        let max_scroll = content_height.saturating_sub(viewport_height);
        let scroll = table_state.offset().min(max_scroll);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(None)
            .end_symbol(None)
            .track_style(theme::dim())
            .thumb_style(theme::title());
        let scroll_positions = max_scroll.saturating_add(1);
        let mut scrollbar_state = ScrollbarState::new(scroll_positions)
            .viewport_content_length(viewport_height)
            .position(scroll);
        frame.render_stateful_widget(scrollbar, scrollbar_area, &mut scrollbar_state);
    }
}
