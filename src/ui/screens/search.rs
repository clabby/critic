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
    layout::{Constraint, Layout, Rect},
    text::{Line, Span},
    widgets::{
        Block, Borders, List, ListItem, ListState, Scrollbar, ScrollbarOrientation, ScrollbarState,
    },
};

const DATE_COL_WIDTH: usize = 5;
const STATUS_COL_WIDTH: usize = 1;
const MIN_AUTHOR_COL_WIDTH: usize = 4;
const MAX_AUTHOR_COL_WIDTH: usize = 18;
const MIN_TITLE_COL_WIDTH: usize = 12;
const COLUMN_SPACER_COUNT: usize = 4;
const LIST_PREFIX_WIDTH: usize = 2;

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

    let number_col_width = state
        .search_results
        .iter()
        .filter_map(|index| state.pull_requests.get(*index))
        .map(|pull| format!("#{}", pull.number).chars().count())
        .max()
        .unwrap_or(2);

    let metadata_width = DATE_COL_WIDTH + STATUS_COL_WIDTH + number_col_width + COLUMN_SPACER_COUNT;
    let row_width = usize::from(list_area.width).saturating_sub(LIST_PREFIX_WIDTH);
    let available_width = row_width.saturating_sub(metadata_width);
    let max_author_by_min_title = available_width.saturating_sub(MIN_TITLE_COL_WIDTH);
    let max_author_by_ratio = available_width / 3;
    let max_author_that_fits = max_author_by_min_title.min(max_author_by_ratio);
    let author_col_width = max_author_that_fits.min(MAX_AUTHOR_COL_WIDTH);
    let author_col_width = if author_col_width >= MIN_AUTHOR_COL_WIDTH {
        author_col_width
    } else {
        max_author_that_fits
    };

    let items: Vec<ListItem<'static>> = if state.search_results.is_empty() {
        vec![ListItem::new(Line::from(vec![Span::styled(
            "No open pull requests match this query.",
            theme::dim(),
        )]))]
    } else {
        state
            .search_results
            .iter()
            .filter_map(|index| state.pull_requests.get(*index))
            .map(|pull| {
                let status_text = match pull.review_status {
                    Some(PullRequestReviewStatus::Approved) => "A",
                    Some(PullRequestReviewStatus::ChangesRequested) => "R",
                    None => "",
                };

                let author = fit_column_left(&pull.author, author_col_width);
                let timestamp = fit_column_left(
                    &short_timestamp(&pull.updated_at, pull.updated_at_unix_ms),
                    DATE_COL_WIDTH,
                );
                let status = fit_column_left(status_text, STATUS_COL_WIDTH);
                let number = fit_column_right(&format!("#{}", pull.number), number_col_width);

                let status_style = match pull.review_status {
                    Some(PullRequestReviewStatus::Approved) => theme::resolved_thread(),
                    Some(PullRequestReviewStatus::ChangesRequested) => theme::error(),
                    None => theme::dim(),
                };

                ListItem::new(Line::from(vec![
                    Span::styled(author, theme::dim()),
                    Span::raw(" "),
                    Span::styled(timestamp, theme::dim()),
                    Span::raw(" "),
                    Span::styled(status, status_style),
                    Span::raw(" "),
                    Span::styled(number, theme::title()),
                    Span::raw(" "),
                    Span::raw(pull.title.clone()),
                ]))
            })
            .collect()
    };

    let list = List::new(items)
        .highlight_style(theme::selected())
        .highlight_symbol("▸ ");

    let mut list_state = ListState::default();
    if !state.search_results.is_empty() {
        list_state.select(Some(state.search_selected));
    }

    frame.render_stateful_widget(list, list_area, &mut list_state);

    let viewport_height = usize::from(list_area.height);
    let content_height = state.search_results.len();

    if content_height > viewport_height
        && let Some(scrollbar_area) = scrollbar_area
    {
        let max_scroll = content_height.saturating_sub(viewport_height);
        let scroll = list_state.offset().min(max_scroll);
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

fn fit_column_left(value: &str, width: usize) -> String {
    let fitted = truncate_to_width(value, width);
    let fitted_len = fitted.chars().count();
    if fitted_len >= width {
        return fitted;
    }

    format!("{fitted}{}", " ".repeat(width - fitted_len))
}

fn fit_column_right(value: &str, width: usize) -> String {
    let fitted = truncate_to_width(value, width);
    let fitted_len = fitted.chars().count();
    if fitted_len >= width {
        return fitted;
    }

    format!("{}{fitted}", " ".repeat(width - fitted_len))
}

fn truncate_to_width(value: &str, width: usize) -> String {
    if value.chars().count() <= width {
        return value.to_owned();
    }
    if width == 0 {
        return String::new();
    }
    if width == 1 {
        return "…".to_owned();
    }
    if width <= 3 {
        return value.chars().take(width).collect();
    }

    let mut out = String::new();
    for ch in value.chars().take(width - 1) {
        out.push(ch);
    }
    out.push('…');
    out
}
