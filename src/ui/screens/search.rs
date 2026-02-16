//! Pull request fuzzy-search screen renderer.

use crate::app::state::AppState;
use crate::ui::components::shared::short_timestamp;
use crate::ui::theme;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};

pub fn render(frame: &mut Frame<'_>, area: Rect, state: &AppState) {
    let rows = Layout::vertical([Constraint::Length(3), Constraint::Min(6)]).split(area);

    render_search_box(frame, rows[0], state);
    render_results(frame, rows[1], state);
}

fn render_search_box(frame: &mut Frame<'_>, area: Rect, state: &AppState) {
    let block = Block::default()
        .title(Span::styled(" PR Search ", theme::title()))
        .borders(Borders::ALL)
        .border_style(theme::border());

    let text = if state.search_query.is_empty() {
        vec![Line::from(vec![
            Span::raw("  query: "),
            Span::styled("(type to filter open pull requests)", theme::dim()),
        ])]
    } else {
        vec![Line::from(vec![
            Span::raw("  query: "),
            Span::styled(state.search_query.clone(), Style::default()),
        ])]
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
                ListItem::new(Line::from(vec![
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
        .block(block)
        .highlight_style(theme::selected())
        .highlight_symbol("▸ ");

    let mut list_state = ListState::default();
    if !state.search_results.is_empty() {
        list_state.select(Some(state.search_selected));
    }

    frame.render_stateful_widget(list, area, &mut list_state);
}
