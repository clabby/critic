use crate::ui::theme;
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

pub struct SearchBoxProps<'a> {
    pub title: &'a str,
    pub query: &'a str,
    pub focused: bool,
    pub focused_placeholder: &'a str,
    pub unfocused_placeholder: &'a str,
}

pub fn render(frame: &mut Frame<'_>, area: Rect, props: SearchBoxProps<'_>) {
    let title_style = if props.focused {
        theme::info()
    } else {
        theme::title()
    };
    let block = Block::default()
        .title(Span::styled(props.title, title_style))
        .borders(Borders::ALL)
        .border_style(if props.focused {
            theme::open_thread()
        } else {
            theme::border()
        });

    let line = if props.query.is_empty() {
        Line::from(vec![
            Span::raw("  query: "),
            Span::styled(
                if props.focused {
                    props.focused_placeholder
                } else {
                    props.unfocused_placeholder
                },
                theme::dim(),
            ),
        ])
    } else {
        let mut text = vec![
            Span::raw("  query: "),
            Span::styled(props.query.to_owned(), theme::text()),
        ];
        if props.focused {
            text.push(Span::styled(" |", theme::open_thread()));
        }
        Line::from(text)
    };

    frame.render_widget(Paragraph::new(line).block(block), area);
}
