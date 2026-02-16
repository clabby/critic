//! Pull request fuzzy-search screen renderer.

use crate::{
    app::state::AppState,
    domain::PullRequestReviewStatus,
    ui::{components::shared::short_timestamp, theme},
};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{
        Block, Borders, List, ListItem, ListState, Paragraph, Scrollbar, ScrollbarOrientation,
        ScrollbarState,
    },
};

pub fn render(frame: &mut Frame<'_>, area: Rect, state: &AppState) {
    let rows = Layout::vertical([Constraint::Length(3), Constraint::Min(6)]).split(area);

    render_search_box(frame, rows[0], state);
    render_results(frame, rows[1], state);
}

fn render_search_box(frame: &mut Frame<'_>, area: Rect, state: &AppState) {
    let focused = state.is_search_focused();
    let title_style = if focused {
        theme::info()
    } else {
        theme::title()
    };
    let block = Block::default()
        .title(Span::styled(" PR Search ", title_style))
        .borders(Borders::ALL)
        .border_style(if focused {
            theme::open_thread()
        } else {
            theme::border()
        });

    let text = if state.search_query.is_empty() {
        vec![Line::from(vec![
            Span::raw("  query: "),
            Span::styled(
                if focused {
                    "(type to filter open pull requests)"
                } else {
                    "(press [s] to focus search)"
                },
                theme::dim(),
            ),
        ])]
    } else {
        let mut line = vec![
            Span::raw("  query: "),
            Span::styled(state.search_query.clone(), Style::default()),
        ];
        if focused {
            line.push(Span::styled(" |", theme::open_thread()));
        }
        vec![Line::from(line)]
    };

    frame.render_widget(Paragraph::new(text).block(block), area);
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
                let status_icon = match pull.review_status {
                    Some(PullRequestReviewStatus::Approved) => {
                        Span::styled("✅ ", theme::resolved_thread())
                    }
                    Some(PullRequestReviewStatus::ChangesRequested) => {
                        Span::styled("❌ ", theme::error())
                    }
                    None => Span::raw(""),
                };
                ListItem::new(Line::from(vec![
                    status_icon,
                    Span::styled(format!("#{} ", pull.number), theme::title()),
                    Span::raw(pull.title.clone()),
                    Span::styled(format!("  @{}", pull.author), theme::dim()),
                    Span::styled(
                        format!("  {}", short_timestamp(&pull.updated_at)),
                        theme::dim(),
                    ),
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
