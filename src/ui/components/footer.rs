//! Footer component used for keybinding hints.

use crate::ui::theme;
use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

/// Returns the footer height required to render all hint tokens for the given terminal width.
pub fn required_height(screen_width: u16, hints: &str) -> u16 {
    let width = usize::from(screen_width.max(1));
    wrap_hint_tokens(hints, width).len().max(1) as u16
}

/// Renders keybinding hints in a plain bottom bar.
pub fn render(frame: &mut Frame<'_>, area: Rect, hints: &str) {
    let width = usize::from(area.width.max(1));
    let lines = wrap_hint_tokens(hints, width);
    let text: Vec<Line<'static>> = if lines.is_empty() {
        vec![Line::from(" ")]
    } else {
        lines.iter().map(|line| styled_hint_line(line)).collect()
    };

    frame.render_widget(Paragraph::new(text).alignment(Alignment::Center), area);
}

#[derive(Debug, Clone)]
struct HintToken {
    key: String,
    desc: String,
}

fn wrap_hint_tokens(hints: &str, width: usize) -> Vec<Vec<HintToken>> {
    if hints.is_empty() {
        return Vec::new();
    }

    let width = width.max(1);
    let tokens: Vec<HintToken> = hints
        .split("  ")
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(parse_hint_token)
        .collect();

    if tokens.is_empty() {
        return Vec::new();
    }

    let mut lines = Vec::new();
    let mut current = Vec::<HintToken>::new();
    let mut current_width = 0usize;

    for token in tokens {
        let token_width = token_display_width(&token);
        let separator_width = if current.is_empty() { 0 } else { 2 };
        let projected_width = current_width + separator_width + token_width;

        if projected_width <= width {
            current.push(token);
            current_width = projected_width;
            continue;
        }

        if !current.is_empty() {
            lines.push(current);
            current = Vec::new();
        }

        current_width = token_width.min(width);
        current.push(token);
    }

    if !current.is_empty() {
        lines.push(current);
    }

    lines
}

fn parse_hint_token(token: &str) -> HintToken {
    let token = token.trim();
    if token.starts_with('[') {
        if let Some(end) = token.find(']') {
            let key = token[..=end].to_owned();
            let desc = token[end + 1..].trim().to_owned();
            return HintToken { key, desc };
        }
    }

    HintToken {
        key: String::new(),
        desc: token.to_owned(),
    }
}

fn token_display_width(token: &HintToken) -> usize {
    let key_width = token.key.chars().count();
    let desc_width = token.desc.chars().count();
    if key_width == 0 {
        desc_width
    } else if desc_width == 0 {
        key_width
    } else {
        key_width + 1 + desc_width
    }
}

fn styled_hint_line(tokens: &[HintToken]) -> Line<'static> {
    let mut spans = Vec::<Span<'static>>::new();
    for (index, token) in tokens.iter().enumerate() {
        if index > 0 {
            spans.push(Span::styled("  ", theme::dim()));
        }
        if !token.key.is_empty() {
            spans.push(Span::styled(token.key.clone(), theme::open_thread()));
        }
        if !token.desc.is_empty() {
            if !token.key.is_empty() {
                spans.push(Span::styled(" ", theme::dim()));
            }
            spans.push(Span::styled(token.desc.clone(), theme::dim()));
        }
    }
    if spans.is_empty() {
        spans.push(Span::raw(" "));
    }
    Line::from(spans)
}
