//! Pull request fuzzy-search screen renderer.

use crate::{
    app::state::AppState,
    domain::PullRequestReviewStatus,
    ui::{
        components::{search_box, shared::short_timestamp},
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

const DATE_COL_WIDTH: u16 = 12;
const STATUS_COL_WIDTH: u16 = 1;
const MIN_AUTHOR_COL_WIDTH: u16 = 4;
const MAX_AUTHOR_COL_WIDTH: u16 = 18;
const MIN_TITLE_COL_WIDTH: u16 = 12;
const COLUMN_SPACING: u16 = 1;

pub fn render(frame: &mut Frame<'_>, area: Rect, state: &AppState) {
    let rows = Layout::vertical([Constraint::Length(3), Constraint::Min(6)]).split(area);

    render_search_box(frame, rows[0], state);
    render_results(frame, rows[1], state);
}

fn render_search_box(frame: &mut Frame<'_>, area: Rect, state: &AppState) {
    search_box::render(
        frame,
        area,
        search_box::SearchBoxProps {
            title: " PR Search ",
            query: state.search_query(),
            focused: state.is_search_focused(),
            focused_placeholder: "(type to filter open pull requests)",
            unfocused_placeholder: "(press [s] to focus search)",
        },
    );
}

fn render_results(frame: &mut Frame<'_>, area: Rect, state: &AppState) {
    let block = Block::default()
        .title(Span::styled(
            format!(" Open Pull Requests ({}) ", state.search_results.len()),
            theme::title(),
        ))
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
    let fixed = DATE_COL_WIDTH + STATUS_COL_WIDTH + number_col_width;
    let overhead = COLUMN_SPACING * 4 + 2;
    let available = list_area.width.saturating_sub(fixed + overhead);
    let max_author_by_min_title = available.saturating_sub(MIN_TITLE_COL_WIDTH);
    let max_author_by_ratio = available / 3;
    let max_author_that_fits = max_author_by_min_title.min(max_author_by_ratio);
    let author_col_width = if max_author_that_fits.min(MAX_AUTHOR_COL_WIDTH) >= MIN_AUTHOR_COL_WIDTH
    {
        max_author_that_fits.min(MAX_AUTHOR_COL_WIDTH)
    } else {
        max_author_that_fits
    };

    let widths = [
        Constraint::Length(author_col_width),
        Constraint::Length(DATE_COL_WIDTH),
        Constraint::Length(STATUS_COL_WIDTH),
        Constraint::Length(number_col_width),
        Constraint::Fill(1),
    ];

    let rows: Vec<Row<'_>> = state
        .search_results
        .iter()
        .filter_map(|index| state.pull_requests.get(*index))
        .map(|pull| {
            let status_text = match pull.review_status {
                Some(PullRequestReviewStatus::Approved) => "A",
                Some(PullRequestReviewStatus::ChangesRequested) => "R",
                None => "",
            };

            let status_style = match pull.review_status {
                Some(PullRequestReviewStatus::Approved) => theme::resolved_thread(),
                Some(PullRequestReviewStatus::ChangesRequested) => theme::error(),
                None => theme::dim(),
            };

            Row::new([
                Cell::new(Span::styled(pull.author.clone(), theme::dim())),
                Cell::new(Span::styled(
                    short_timestamp(pull.updated_at_unix_ms),
                    theme::dim(),
                )),
                Cell::new(Span::styled(status_text, status_style)),
                Cell::new(
                    Line::styled(format!("#{}", pull.number), theme::title())
                        .alignment(Alignment::Right),
                ),
                Cell::new(pull.title.clone()),
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
