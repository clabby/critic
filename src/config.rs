//! User configuration loading from `~/.review-tui/config.toml`.

use crate::ui::theme::ThemePalette;
use anyhow::{Context, Result, anyhow};
use ratatui::style::Color;
use serde::Deserialize;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const CONFIG_DIR: &str = ".review-tui";
const CONFIG_FILE: &str = "config.toml";

const DEFAULT_CONFIG_TOML: &str = r##"# review-tui configuration
# Colors accept `#RRGGBB` or named ANSI colors (e.g. "yellow", "dark_gray").

[theme]
border = "#c47832"
title = "#ebaa5a"
dim = "dark_gray"
text = "#d2d2c8"
selected_fg = "black"
selected_bg = "#e2b45c"
issue = "#e7b258"
open_thread = "yellow"
resolved_thread = "green"
error = "red"
info = "cyan"
link = "cyan"
inline_code_fg = "yellow"
inline_code_bg = "#282828"
section_title = "light_yellow"
author = "light_blue"
outdated = "yellow"
diff_header = "cyan"
diff_add = "green"
diff_remove = "red"
diff_context = "#bebeb4"
gauge_label = "black"
gauge_fill = "#f5cd52"
gauge_empty = "#5e501e"
code_padding_fg = "black"
"##;

/// Application configuration loaded from disk.
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub theme: ThemePalette,
}

/// Returns the config file path and creates default config if missing.
pub fn ensure_config_file() -> Result<PathBuf> {
    let path = config_path()?;
    ensure_default_config(&path)?;
    Ok(path)
}

/// Loads configuration from `~/.review-tui/config.toml`, creating defaults if missing.
pub fn load_or_create() -> Result<AppConfig> {
    let path = ensure_config_file()?;
    let content = fs::read_to_string(&path)
        .with_context(|| format!("failed to read config file at {}", path.display()))?;

    let raw: RawConfig = toml::from_str(&content)
        .with_context(|| format!("failed to parse TOML in {}", path.display()))?;
    let theme = raw.theme.into_theme()?;

    Ok(AppConfig { theme })
}

fn config_path() -> Result<PathBuf> {
    let home =
        env::var_os("HOME").ok_or_else(|| anyhow!("HOME environment variable is not set"))?;
    Ok(PathBuf::from(home).join(CONFIG_DIR).join(CONFIG_FILE))
}

fn ensure_default_config(path: &Path) -> Result<()> {
    if path.exists() {
        return Ok(());
    }

    let dir = path
        .parent()
        .ok_or_else(|| anyhow!("invalid config path: {}", path.display()))?;
    fs::create_dir_all(dir)
        .with_context(|| format!("failed to create config directory {}", dir.display()))?;
    fs::write(path, DEFAULT_CONFIG_TOML)
        .with_context(|| format!("failed to write default config file {}", path.display()))?;
    Ok(())
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct RawConfig {
    theme: RawTheme,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct RawTheme {
    border: Option<String>,
    title: Option<String>,
    dim: Option<String>,
    text: Option<String>,
    selected_fg: Option<String>,
    selected_bg: Option<String>,
    issue: Option<String>,
    open_thread: Option<String>,
    resolved_thread: Option<String>,
    error: Option<String>,
    info: Option<String>,
    link: Option<String>,
    inline_code_fg: Option<String>,
    inline_code_bg: Option<String>,
    section_title: Option<String>,
    author: Option<String>,
    outdated: Option<String>,
    diff_header: Option<String>,
    diff_add: Option<String>,
    diff_remove: Option<String>,
    diff_context: Option<String>,
    gauge_label: Option<String>,
    gauge_fill: Option<String>,
    gauge_empty: Option<String>,
    code_padding_fg: Option<String>,
}

impl RawTheme {
    fn into_theme(self) -> Result<ThemePalette> {
        let defaults = ThemePalette::default();

        Ok(ThemePalette {
            border: parse_or_default(self.border, defaults.border, "theme.border")?,
            title: parse_or_default(self.title, defaults.title, "theme.title")?,
            dim: parse_or_default(self.dim, defaults.dim, "theme.dim")?,
            text: parse_or_default(self.text, defaults.text, "theme.text")?,
            selected_fg: parse_or_default(
                self.selected_fg,
                defaults.selected_fg,
                "theme.selected_fg",
            )?,
            selected_bg: parse_or_default(
                self.selected_bg,
                defaults.selected_bg,
                "theme.selected_bg",
            )?,
            issue: parse_or_default(self.issue, defaults.issue, "theme.issue")?,
            open_thread: parse_or_default(
                self.open_thread,
                defaults.open_thread,
                "theme.open_thread",
            )?,
            resolved_thread: parse_or_default(
                self.resolved_thread,
                defaults.resolved_thread,
                "theme.resolved_thread",
            )?,
            error: parse_or_default(self.error, defaults.error, "theme.error")?,
            info: parse_or_default(self.info, defaults.info, "theme.info")?,
            link: parse_or_default(self.link, defaults.link, "theme.link")?,
            inline_code_fg: parse_or_default(
                self.inline_code_fg,
                defaults.inline_code_fg,
                "theme.inline_code_fg",
            )?,
            inline_code_bg: parse_or_default(
                self.inline_code_bg,
                defaults.inline_code_bg,
                "theme.inline_code_bg",
            )?,
            section_title: parse_or_default(
                self.section_title,
                defaults.section_title,
                "theme.section_title",
            )?,
            author: parse_or_default(self.author, defaults.author, "theme.author")?,
            outdated: parse_or_default(self.outdated, defaults.outdated, "theme.outdated")?,
            diff_header: parse_or_default(
                self.diff_header,
                defaults.diff_header,
                "theme.diff_header",
            )?,
            diff_add: parse_or_default(self.diff_add, defaults.diff_add, "theme.diff_add")?,
            diff_remove: parse_or_default(
                self.diff_remove,
                defaults.diff_remove,
                "theme.diff_remove",
            )?,
            diff_context: parse_or_default(
                self.diff_context,
                defaults.diff_context,
                "theme.diff_context",
            )?,
            gauge_label: parse_or_default(
                self.gauge_label,
                defaults.gauge_label,
                "theme.gauge_label",
            )?,
            gauge_fill: parse_or_default(self.gauge_fill, defaults.gauge_fill, "theme.gauge_fill")?,
            gauge_empty: parse_or_default(
                self.gauge_empty,
                defaults.gauge_empty,
                "theme.gauge_empty",
            )?,
            code_padding_fg: parse_or_default(
                self.code_padding_fg,
                defaults.code_padding_fg,
                "theme.code_padding_fg",
            )?,
        })
    }
}

fn parse_or_default(value: Option<String>, default: Color, field: &str) -> Result<Color> {
    match value {
        Some(raw) => parse_color(raw.trim())
            .with_context(|| format!("invalid color value for `{field}`: {raw}")),
        None => Ok(default),
    }
}

fn parse_color(raw: &str) -> Result<Color> {
    if let Some(hex) = raw.strip_prefix('#') {
        if hex.len() != 6 {
            return Err(anyhow!("hex colors must be in #RRGGBB format"));
        }
        let red = u8::from_str_radix(&hex[0..2], 16).context("invalid red hex channel")?;
        let green = u8::from_str_radix(&hex[2..4], 16).context("invalid green hex channel")?;
        let blue = u8::from_str_radix(&hex[4..6], 16).context("invalid blue hex channel")?;
        return Ok(Color::Rgb(red, green, blue));
    }

    let normalized = raw.trim().to_ascii_lowercase().replace(['-', ' '], "_");
    let color = match normalized.as_str() {
        "reset" => Color::Reset,
        "black" => Color::Black,
        "red" => Color::Red,
        "green" => Color::Green,
        "yellow" => Color::Yellow,
        "blue" => Color::Blue,
        "magenta" => Color::Magenta,
        "cyan" => Color::Cyan,
        "gray" | "grey" => Color::Gray,
        "dark_gray" | "dark_grey" => Color::DarkGray,
        "light_red" => Color::LightRed,
        "light_green" => Color::LightGreen,
        "light_yellow" => Color::LightYellow,
        "light_blue" => Color::LightBlue,
        "light_magenta" => Color::LightMagenta,
        "light_cyan" => Color::LightCyan,
        "white" => Color::White,
        _ => return Err(anyhow!("unsupported color format")),
    };

    Ok(color)
}

#[cfg(test)]
mod tests {
    use super::parse_color;
    use ratatui::style::Color;

    #[test]
    fn parse_color_supports_hex() {
        assert_eq!(
            parse_color("#112233").unwrap(),
            Color::Rgb(0x11, 0x22, 0x33)
        );
    }

    #[test]
    fn parse_color_supports_named_values() {
        assert_eq!(parse_color("light_yellow").unwrap(), Color::LightYellow);
        assert_eq!(parse_color("dark-gray").unwrap(), Color::DarkGray);
    }
}
