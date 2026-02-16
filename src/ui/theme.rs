//! Shared styles for the TUI.

use ratatui::style::{Color, Modifier, Style};

pub fn border() -> Style {
    Style::default().fg(Color::Rgb(196, 120, 50))
}

pub fn title() -> Style {
    Style::default()
        .fg(Color::Rgb(235, 170, 90))
        .add_modifier(Modifier::BOLD)
}

pub fn dim() -> Style {
    Style::default().fg(Color::DarkGray)
}

pub fn selected() -> Style {
    Style::default()
        .fg(Color::Black)
        .bg(Color::Rgb(226, 180, 92))
}

pub fn issue() -> Style {
    Style::default().fg(Color::Rgb(231, 178, 88))
}

pub fn open_thread() -> Style {
    Style::default().fg(Color::Yellow)
}

pub fn resolved_thread() -> Style {
    Style::default().fg(Color::Green)
}

pub fn error() -> Style {
    Style::default().fg(Color::Red)
}

pub fn info() -> Style {
    Style::default().fg(Color::Cyan)
}
