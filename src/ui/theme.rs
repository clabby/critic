//! Shared styles for the TUI.

use ratatui::style::{Color, Modifier, Style};
use std::sync::{OnceLock, RwLock};

/// Runtime theme palette used by the renderer.
#[derive(Debug, Clone)]
pub struct ThemePalette {
    pub border: Color,
    pub title: Color,
    pub dim: Color,
    pub text: Color,
    pub selected_fg: Color,
    pub selected_bg: Color,
    pub issue: Color,
    pub open_thread: Color,
    pub resolved_thread: Color,
    pub error: Color,
    pub info: Color,
    pub link: Color,
    pub inline_code_fg: Color,
    pub inline_code_bg: Color,
    pub section_title: Color,
    pub author: Color,
    pub outdated: Color,
    pub diff_header: Color,
    pub diff_add: Color,
    pub diff_remove: Color,
    pub diff_context: Color,
    pub gauge_label: Color,
    pub gauge_fill: Color,
    pub gauge_empty: Color,
    pub code_padding_fg: Color,
}

impl Default for ThemePalette {
    fn default() -> Self {
        Self {
            border: Color::Rgb(196, 120, 50),
            title: Color::Rgb(235, 170, 90),
            dim: Color::DarkGray,
            text: Color::Rgb(210, 210, 200),
            selected_fg: Color::Black,
            selected_bg: Color::Rgb(226, 180, 92),
            issue: Color::Rgb(231, 178, 88),
            open_thread: Color::Yellow,
            resolved_thread: Color::Green,
            error: Color::Red,
            info: Color::Cyan,
            link: Color::Cyan,
            inline_code_fg: Color::Yellow,
            inline_code_bg: Color::Rgb(40, 40, 40),
            section_title: Color::LightYellow,
            author: Color::LightBlue,
            outdated: Color::Yellow,
            diff_header: Color::Cyan,
            diff_add: Color::Green,
            diff_remove: Color::Red,
            diff_context: Color::Rgb(190, 190, 180),
            gauge_label: Color::Black,
            gauge_fill: Color::Rgb(245, 205, 82),
            gauge_empty: Color::Rgb(94, 80, 30),
            code_padding_fg: Color::Black,
        }
    }
}

static ACTIVE_THEME: OnceLock<RwLock<ThemePalette>> = OnceLock::new();

fn store() -> &'static RwLock<ThemePalette> {
    ACTIVE_THEME.get_or_init(|| RwLock::new(ThemePalette::default()))
}

fn with_palette<T>(f: impl FnOnce(&ThemePalette) -> T) -> T {
    let guard = store().read().expect("theme lock poisoned");
    f(&guard)
}

/// Installs the active runtime theme palette.
pub fn apply(palette: ThemePalette) {
    if let Ok(mut guard) = store().write() {
        *guard = palette;
    }
}

pub fn border() -> Style {
    with_palette(|theme| Style::default().fg(theme.border))
}

pub fn title() -> Style {
    with_palette(|theme| {
        Style::default()
            .fg(theme.title)
            .add_modifier(Modifier::BOLD)
    })
}

pub fn dim() -> Style {
    with_palette(|theme| Style::default().fg(theme.dim))
}

pub fn text() -> Style {
    with_palette(|theme| Style::default().fg(theme.text))
}

pub fn selected() -> Style {
    with_palette(|theme| Style::default().fg(theme.selected_fg).bg(theme.selected_bg))
}

pub fn issue() -> Style {
    with_palette(|theme| Style::default().fg(theme.issue))
}

pub fn open_thread() -> Style {
    with_palette(|theme| Style::default().fg(theme.open_thread))
}

pub fn resolved_thread() -> Style {
    with_palette(|theme| Style::default().fg(theme.resolved_thread))
}

pub fn error() -> Style {
    with_palette(|theme| Style::default().fg(theme.error))
}

pub fn info() -> Style {
    with_palette(|theme| Style::default().fg(theme.info))
}

pub fn link_color() -> Color {
    with_palette(|theme| theme.link)
}

pub fn inline_code() -> Style {
    with_palette(|theme| {
        Style::default()
            .fg(theme.inline_code_fg)
            .bg(theme.inline_code_bg)
            .add_modifier(Modifier::BOLD)
    })
}

pub fn section_title() -> Style {
    with_palette(|theme| {
        Style::default()
            .fg(theme.section_title)
            .add_modifier(Modifier::BOLD)
    })
}

pub fn author() -> Style {
    with_palette(|theme| Style::default().fg(theme.author))
}

pub fn outdated() -> Style {
    with_palette(|theme| Style::default().fg(theme.outdated))
}

pub fn diff_header() -> Style {
    with_palette(|theme| Style::default().fg(theme.diff_header))
}

pub fn diff_add() -> Style {
    with_palette(|theme| Style::default().fg(theme.diff_add))
}

pub fn diff_remove() -> Style {
    with_palette(|theme| Style::default().fg(theme.diff_remove))
}

pub fn diff_context() -> Style {
    with_palette(|theme| Style::default().fg(theme.diff_context))
}

pub fn gauge_label() -> Style {
    with_palette(|theme| Style::default().fg(theme.gauge_label))
}

pub fn gauge_fill() -> Style {
    with_palette(|theme| Style::default().fg(theme.gauge_fill).bg(theme.gauge_empty))
}

pub fn gauge_empty() -> Style {
    with_palette(|theme| Style::default().fg(theme.gauge_label).bg(theme.gauge_empty))
}

pub fn code_padding() -> Style {
    with_palette(|theme| Style::default().fg(theme.code_padding_fg))
}
