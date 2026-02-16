//! Review split-pane screen renderer.

use crate::app::state::AppState;
use crate::domain::{CommentRef, ListNodeKind};
use crate::render::markdown::MarkdownRenderer;
use crate::render::thread::{render_issue_preview, render_thread_preview};
use crate::ui::components::shared::short_preview;
use crate::ui::theme;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};

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

    let panes =
        Layout::horizontal([Constraint::Percentage(47), Constraint::Percentage(53)]).split(area);

    render_left_pane(frame, panes[0], review);
    render_right_pane(frame, panes[1], review, markdown);
}

fn render_left_pane(
    frame: &mut Frame<'_>,
    area: Rect,
    review: &crate::app::state::ReviewScreenState,
) {
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
    review: &crate::app::state::ReviewScreenState,
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
            (_, CommentRef::Review(_)) => {
                if let Some(root) = review.selected_root_thread() {
                    render_thread_preview(markdown, node, root, review.selected_reply_draft())
                } else {
                    vec![Line::from(vec![Span::styled(
                        "Thread not found for selected row.",
                        Style::default().fg(ratatui::style::Color::Red),
                    )])]
                }
            }
            _ => vec![Line::from("Select a row to preview comment details.")],
        }
    } else {
        vec![Line::from("Select a row to preview comment details.")]
    };

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((review.right_scroll, 0));

    frame.render_widget(paragraph, area);
}
