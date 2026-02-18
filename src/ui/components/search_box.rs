use crate::ui::theme;
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

pub struct SearchBoxProps<'a> {
    pub title: &'a str,
    pub title_line: Option<Line<'a>>,
    pub query: &'a str,
    pub focused: bool,
    pub focused_placeholder: &'a str,
    pub unfocused_placeholder: &'a str,
    pub focused_right_hint: Option<&'a str>,
}

pub fn render(frame: &mut Frame<'_>, area: Rect, props: SearchBoxProps<'_>) {
    let title_style = if props.focused {
        theme::info()
    } else {
        theme::title()
    };
    let title = props
        .title_line
        .unwrap_or_else(|| Line::from(Span::styled(props.title, title_style)));
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(if props.focused {
            theme::open_thread()
        } else {
            theme::border()
        });

    let line = if props.query.is_empty() {
        let mut value = if props.focused {
            props.focused_placeholder.to_owned()
        } else {
            props.unfocused_placeholder.to_owned()
        };
        if props.focused {
            value.push('|');
        }
        let left_text = format!("  {value}");

        line_with_optional_right_hint(
            &left_text,
            theme::dim(),
            props.focused,
            props.focused_right_hint,
            usize::from(area.width),
        )
    } else {
        let mut left_text = format!("  {}", props.query);
        if props.focused {
            left_text.push('|');
        }

        line_with_optional_right_hint(
            &left_text,
            theme::text(),
            props.focused,
            props.focused_right_hint,
            usize::from(area.width),
        )
    };

    frame.render_widget(Paragraph::new(line).block(block), area);
}

fn line_with_optional_right_hint(
    left_text: &str,
    left_style: Style,
    focused: bool,
    focused_right_hint: Option<&str>,
    total_width: usize,
) -> Line<'static> {
    if !focused || focused_right_hint.is_none() {
        return Line::from(vec![Span::styled(left_text.to_owned(), left_style)]);
    }
    let hint = focused_right_hint.unwrap_or_default();

    let hint_text = format!(" {hint}");
    let inner_width = total_width.saturating_sub(2);
    let left_len = left_text.chars().count();
    let hint_len = hint_text.chars().count();

    if inner_width <= left_len + hint_len + 1 {
        return Line::from(vec![
            Span::styled(left_text.to_owned(), left_style),
            Span::styled(hint_text, theme::info()),
        ]);
    }

    let gap = inner_width.saturating_sub(left_len + hint_len);
    Line::from(vec![
        Span::styled(left_text.to_owned(), left_style),
        Span::raw(" ".repeat(gap)),
        Span::styled(hint_text, theme::info()),
    ])
}
