//! Shared styles for the TUI.

use ratatui::style::{Color, Modifier, Style};
use std::sync::{OnceLock, RwLock};

/// Active UI theme mode.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ThemeMode {
    Dark,
    Light,
}

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
}

impl Default for ThemePalette {
    fn default() -> Self {
        Self {
            border: Color::DarkGray,
            title: Color::LightBlue,
            dim: Color::DarkGray,
            text: Color::Gray,
            selected_fg: Color::Black,
            selected_bg: Color::Cyan,
            issue: Color::Cyan,
            open_thread: Color::Cyan,
            resolved_thread: Color::Green,
            error: Color::Red,
            info: Color::Cyan,
            link: Color::LightBlue,
            inline_code_fg: Color::Cyan,
            inline_code_bg: Color::Black,
            section_title: Color::LightBlue,
            author: Color::LightBlue,
            outdated: Color::Cyan,
            diff_header: Color::Cyan,
            diff_add: Color::Green,
            diff_remove: Color::Red,
            diff_context: Color::Gray,
            gauge_label: Color::Black,
            gauge_fill: Color::Cyan,
            gauge_empty: Color::DarkGray,
        }
    }
}

impl ThemePalette {
    /// Default light palette optimized for terminal readability.
    pub fn light_default() -> Self {
        Self {
            border: Color::DarkGray,
            title: Color::Blue,
            dim: Color::DarkGray,
            text: Color::Black,
            selected_fg: Color::Black,
            selected_bg: Color::Cyan,
            issue: Color::Cyan,
            open_thread: Color::Blue,
            resolved_thread: Color::Green,
            error: Color::Red,
            info: Color::Blue,
            link: Color::Blue,
            inline_code_fg: Color::Blue,
            inline_code_bg: Color::White,
            section_title: Color::Black,
            author: Color::Blue,
            outdated: Color::Cyan,
            diff_header: Color::Blue,
            diff_add: Color::Green,
            diff_remove: Color::Red,
            diff_context: Color::DarkGray,
            gauge_label: Color::Black,
            gauge_fill: Color::Cyan,
            gauge_empty: Color::Gray,
        }
    }
}

#[derive(Debug, Clone)]
struct ActiveTheme {
    palette: ThemePalette,
    mode: ThemeMode,
    terminal_background_rgb: Option<(u8, u8, u8)>,
}

impl Default for ActiveTheme {
    fn default() -> Self {
        Self {
            palette: ThemePalette::default(),
            mode: ThemeMode::Dark,
            terminal_background_rgb: None,
        }
    }
}

static ACTIVE_THEME: OnceLock<RwLock<ActiveTheme>> = OnceLock::new();

fn store() -> &'static RwLock<ActiveTheme> {
    ACTIVE_THEME.get_or_init(|| RwLock::new(ActiveTheme::default()))
}

fn with_palette<T>(f: impl FnOnce(&ThemePalette) -> T) -> T {
    let guard = store().read().expect("theme lock poisoned");
    f(&guard.palette)
}

/// Installs the active runtime theme palette and mode.
pub fn apply(
    palette: ThemePalette,
    mode: ThemeMode,
    terminal_background_rgb: Option<(u8, u8, u8)>,
) {
    if let Ok(mut guard) = store().write() {
        *guard = ActiveTheme {
            palette,
            mode,
            terminal_background_rgb,
        };
    }
}

pub fn blend_with_terminal_bg(overlay: Color, alpha: f32) -> Color {
    let alpha = alpha.clamp(0.0, 1.0);
    let (bg_r, bg_g, bg_b) = terminal_background_rgb();
    let (ov_r, ov_g, ov_b) = color_to_rgb(overlay).unwrap_or((bg_r, bg_g, bg_b));

    let blend = |bg: u8, fg: u8| -> u8 {
        let bg = f32::from(bg);
        let fg = f32::from(fg);
        ((bg * (1.0 - alpha) + fg * alpha).round() as i32).clamp(0, 255) as u8
    };

    Color::Rgb(blend(bg_r, ov_r), blend(bg_g, ov_g), blend(bg_b, ov_b))
}

fn terminal_background_rgb() -> (u8, u8, u8) {
    let guard = store().read().expect("theme lock poisoned");
    match (guard.terminal_background_rgb, guard.mode) {
        (Some(rgb), _) => rgb,
        (None, ThemeMode::Dark) => (0, 0, 0),
        (None, ThemeMode::Light) => (255, 255, 255),
    }
}

fn color_to_rgb(color: Color) -> Option<(u8, u8, u8)> {
    match color {
        Color::Reset => None,
        Color::Black => Some((0, 0, 0)),
        Color::Red => Some((205, 0, 0)),
        Color::Green => Some((0, 205, 0)),
        Color::Yellow => Some((205, 205, 0)),
        Color::Blue => Some((0, 0, 238)),
        Color::Magenta => Some((205, 0, 205)),
        Color::Cyan => Some((0, 205, 205)),
        Color::Gray => Some((229, 229, 229)),
        Color::DarkGray => Some((127, 127, 127)),
        Color::LightRed => Some((255, 0, 0)),
        Color::LightGreen => Some((0, 255, 0)),
        Color::LightYellow => Some((255, 255, 0)),
        Color::LightBlue => Some((92, 92, 255)),
        Color::LightMagenta => Some((255, 0, 255)),
        Color::LightCyan => Some((0, 255, 255)),
        Color::White => Some((255, 255, 255)),
        Color::Rgb(r, g, b) => Some((r, g, b)),
        Color::Indexed(index) => Some(indexed_to_rgb(index)),
    }
}

fn indexed_to_rgb(index: u8) -> (u8, u8, u8) {
    const ANSI16: [(u8, u8, u8); 16] = [
        (0, 0, 0),
        (205, 0, 0),
        (0, 205, 0),
        (205, 205, 0),
        (0, 0, 238),
        (205, 0, 205),
        (0, 205, 205),
        (229, 229, 229),
        (127, 127, 127),
        (255, 0, 0),
        (0, 255, 0),
        (255, 255, 0),
        (92, 92, 255),
        (255, 0, 255),
        (0, 255, 255),
        (255, 255, 255),
    ];

    if index < 16 {
        return ANSI16[index as usize];
    }
    if (16..=231).contains(&index) {
        let value = index - 16;
        let r = value / 36;
        let g = (value % 36) / 6;
        let b = value % 6;
        let to_component = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
        return (to_component(r), to_component(g), to_component(b));
    }

    let gray = 8 + (index.saturating_sub(232)) * 10;
    (gray, gray, gray)
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

pub fn strong_text() -> Style {
    with_palette(|theme| Style::default().fg(theme.text).add_modifier(Modifier::BOLD))
}

pub fn selected() -> Style {
    with_palette(|theme| Style::default().fg(theme.selected_fg).bg(theme.selected_bg))
}

pub fn selected_muted() -> Style {
    with_palette(|theme| {
        Style::default()
            .fg(theme.text)
            .bg(blend_with_terminal_bg(theme.dim, 0.35))
    })
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
