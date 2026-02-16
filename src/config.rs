//! User configuration loading from `~/.critic/config.toml`.

use crate::ui::theme::{ThemeMode, ThemePalette};
use anyhow::{Context, Result, anyhow};
use dark_light::Mode;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

const CONFIG_DIR: &str = ".critic";
const CONFIG_FILE: &str = "config.toml";

const DEFAULT_CONFIG_HEADER: &str = r##"# critic configuration
# Set `theme.mode` to one of: "auto", "dark", "light".
"##;

/// Application configuration loaded from disk.
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub theme_preference: ThemePreference,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            theme_preference: ThemePreference::Auto,
        }
    }
}

/// Preferred theme mode from config.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ThemePreference {
    Auto,
    Dark,
    Light,
}

/// Runtime theme selection after resolving `auto` mode.
#[derive(Debug, Clone)]
pub struct RuntimeThemeConfig {
    pub palette: ThemePalette,
    pub mode: ThemeMode,
}

/// Terminal theme sample from runtime detection.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct TerminalThemeSample {
    pub mode: ThemeMode,
    pub background_rgb: Option<(u8, u8, u8)>,
}

impl AppConfig {
    /// Resolves the effective runtime UI theme.
    pub fn resolve_runtime_theme(&self) -> RuntimeThemeConfig {
        let mode = match self.theme_preference {
            ThemePreference::Auto => detect_terminal_theme_mode().unwrap_or(ThemeMode::Dark),
            ThemePreference::Dark => ThemeMode::Dark,
            ThemePreference::Light => ThemeMode::Light,
        };

        self.resolve_runtime_theme_for_mode(mode)
    }

    /// Builds a runtime theme for a specific mode.
    pub fn resolve_runtime_theme_for_mode(&self, mode: ThemeMode) -> RuntimeThemeConfig {
        match mode {
            ThemeMode::Dark => RuntimeThemeConfig {
                palette: ThemePalette::default(),
                mode,
            },
            ThemeMode::Light => RuntimeThemeConfig {
                palette: ThemePalette::light_default(),
                mode,
            },
        }
    }

    fn to_persisted_config(&self) -> PersistedConfig {
        PersistedConfig {
            theme: PersistedThemeConfig {
                mode: theme_preference_to_string(self.theme_preference).to_owned(),
            },
        }
    }
}

/// Returns the config file path and creates default config if missing.
pub fn ensure_config_file() -> Result<PathBuf> {
    let path = config_path()?;
    ensure_default_config(&path)?;
    Ok(path)
}

/// Loads configuration from `~/.critic/config.toml`, creating defaults if missing.
pub fn load_or_create() -> Result<AppConfig> {
    let path = ensure_config_file()?;
    let content = fs::read_to_string(&path)
        .with_context(|| format!("failed to read config file at {}", path.display()))?;

    parse_app_config(&content)
        .with_context(|| format!("failed to parse TOML in {}", path.display()))
}

fn parse_app_config(content: &str) -> Result<AppConfig> {
    let raw: RawConfig = toml::from_str(&content).context("failed to parse config TOML")?;
    let theme_preference = match raw.theme.mode {
        Some(mode) => parse_theme_preference(mode.trim())
            .with_context(|| format!("invalid value for `theme.mode`: {mode}"))?,
        None => ThemePreference::Auto,
    };

    Ok(AppConfig { theme_preference })
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
    let default_toml = build_default_config_toml()?;
    fs::write(path, default_toml)
        .with_context(|| format!("failed to write default config file {}", path.display()))?;
    Ok(())
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    theme: RawThemeConfig,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
#[serde(deny_unknown_fields)]
struct RawThemeConfig {
    mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct PersistedConfig {
    theme: PersistedThemeConfig,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct PersistedThemeConfig {
    mode: String,
}

fn parse_theme_preference(raw: &str) -> Result<ThemePreference> {
    let normalized = raw.trim().to_ascii_lowercase().replace(['-', ' '], "_");
    match normalized.as_str() {
        "auto" => Ok(ThemePreference::Auto),
        "dark" => Ok(ThemePreference::Dark),
        "light" => Ok(ThemePreference::Light),
        _ => Err(anyhow!("expected one of: auto, dark, light")),
    }
}

fn build_default_config_toml() -> Result<String> {
    let default = AppConfig::default().to_persisted_config();
    let serialized =
        toml::to_string_pretty(&default).context("failed to serialize default config")?;
    Ok(format!("{DEFAULT_CONFIG_HEADER}\n\n{serialized}"))
}

fn theme_preference_to_string(value: ThemePreference) -> &'static str {
    match value {
        ThemePreference::Auto => "auto",
        ThemePreference::Dark => "dark",
        ThemePreference::Light => "light",
    }
}

/// Detects terminal background mode using runtime probes with env fallbacks.
pub fn detect_terminal_theme_mode() -> Option<ThemeMode> {
    detect_terminal_theme_sample().map(|sample| sample.mode)
}

/// Detects terminal theme mode and (if available) terminal background RGB.
pub fn detect_terminal_theme_sample() -> Option<TerminalThemeSample> {
    detect_terminal_theme_sample_passive()
        .or_else(|| detect_with_termbg_sample(Duration::from_millis(120)))
}

/// Detects terminal background mode without active terminal probes.
///
/// This avoids writing query sequences or competing for stdin reads while the
/// TUI event loop is running.
pub fn detect_terminal_theme_mode_passive() -> Option<ThemeMode> {
    detect_terminal_theme_sample_passive().map(|sample| sample.mode)
}

/// Passive terminal theme detection without active OSC probes.
pub fn detect_terminal_theme_sample_passive() -> Option<TerminalThemeSample> {
    detect_from_term_background_env()
        .map(|mode| TerminalThemeSample {
            mode,
            background_rgb: None,
        })
        .or_else(detect_from_colorfgbg_sample)
        .or_else(detect_from_system_theme_sample)
}

/// Detects terminal background mode with a bounded live probe suitable for
/// runtime polling while in TUI mode.
pub fn detect_terminal_theme_mode_live(timeout: Duration) -> Option<ThemeMode> {
    detect_terminal_theme_sample_live(timeout).map(|sample| sample.mode)
}

/// Runtime-safe terminal theme detection that can include terminal background RGB.
pub fn detect_terminal_theme_sample_live(_timeout: Duration) -> Option<TerminalThemeSample> {
    detect_terminal_theme_sample_passive()
}

/// Detects terminal background RGB when available.
pub fn detect_terminal_background_rgb() -> Option<(u8, u8, u8)> {
    detect_from_colorfgbg_sample()
        .and_then(|sample| sample.background_rgb)
        .or_else(|| {
            detect_with_termbg_sample(Duration::from_millis(120))
                .and_then(|sample| sample.background_rgb)
        })
}

/// Runtime-safe terminal background RGB detection.
pub fn detect_terminal_background_rgb_live(_timeout: Duration) -> Option<(u8, u8, u8)> {
    detect_from_colorfgbg_sample().and_then(|sample| sample.background_rgb)
}

fn detect_with_termbg_sample(timeout: Duration) -> Option<TerminalThemeSample> {
    match termbg::rgb(timeout) {
        Ok(rgb) => {
            let mode = theme_mode_from_rgb(rgb.r, rgb.g, rgb.b);
            Some(TerminalThemeSample {
                mode,
                background_rgb: Some((
                    scale_channel_u16_to_u8(rgb.r),
                    scale_channel_u16_to_u8(rgb.g),
                    scale_channel_u16_to_u8(rgb.b),
                )),
            })
        }
        _ => None,
    }
}

fn theme_mode_from_rgb(r: u16, g: u16, b: u16) -> ThemeMode {
    let luma = f64::from(r) * 0.299 + f64::from(g) * 0.587 + f64::from(b) * 0.114;
    if luma > 32768.0 {
        ThemeMode::Light
    } else {
        ThemeMode::Dark
    }
}

fn detect_from_term_background_env() -> Option<ThemeMode> {
    for key in ["TERM_BACKGROUND", "TERMINAL_BACKGROUND"] {
        let Ok(value) = env::var(key) else {
            continue;
        };
        if let Some(mode) = parse_theme_mode_hint(&value) {
            return Some(mode);
        }
    }

    None
}

fn detect_from_system_theme_sample() -> Option<TerminalThemeSample> {
    let mode = match dark_light::detect() {
        Mode::Dark => ThemeMode::Dark,
        Mode::Light => ThemeMode::Light,
        Mode::Default => return None,
    };

    Some(TerminalThemeSample {
        mode,
        background_rgb: None,
    })
}

fn detect_from_colorfgbg_sample() -> Option<TerminalThemeSample> {
    let bg_index = parse_colorfgbg_background_index()?;
    let mode = if bg_index >= 7 {
        ThemeMode::Light
    } else {
        ThemeMode::Dark
    };
    let background_rgb = ansi_256_to_rgb(bg_index);
    Some(TerminalThemeSample {
        mode,
        background_rgb: Some(background_rgb),
    })
}

fn parse_colorfgbg_background_index() -> Option<u8> {
    let value = env::var("COLORFGBG").ok()?;
    let raw_bg = value.rsplit(';').next()?.trim();
    raw_bg.parse::<u8>().ok()
}

fn scale_channel_u16_to_u8(value: u16) -> u8 {
    ((u32::from(value) * 255) / 65535) as u8
}

fn ansi_256_to_rgb(index: u8) -> (u8, u8, u8) {
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

fn parse_theme_mode_hint(value: &str) -> Option<ThemeMode> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "light" => Some(ThemeMode::Light),
        "dark" => Some(ThemeMode::Dark),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AppConfig, ThemePreference, ansi_256_to_rgb, build_default_config_toml,
        detect_from_colorfgbg_sample, parse_app_config, parse_theme_mode_hint,
        parse_theme_preference, theme_mode_from_rgb,
    };
    use crate::ui::theme::ThemeMode;
    use std::env;

    #[test]
    fn parse_theme_preference_accepts_expected_values() {
        assert_eq!(
            parse_theme_preference("auto").unwrap(),
            ThemePreference::Auto
        );
        assert_eq!(
            parse_theme_preference("dark").unwrap(),
            ThemePreference::Dark
        );
        assert_eq!(
            parse_theme_preference("light").unwrap(),
            ThemePreference::Light
        );
    }

    #[test]
    fn parse_theme_mode_hint_accepts_light_dark() {
        assert_eq!(parse_theme_mode_hint("light"), Some(ThemeMode::Light));
        assert_eq!(parse_theme_mode_hint("dark"), Some(ThemeMode::Dark));
        assert_eq!(parse_theme_mode_hint("something-else"), None);
    }

    #[test]
    fn colorfgbg_detection_maps_light_and_dark() {
        unsafe {
            env::set_var("COLORFGBG", "15;0");
        }
        let dark = detect_from_colorfgbg_sample().unwrap();
        assert_eq!(dark.mode, ThemeMode::Dark);
        assert_eq!(dark.background_rgb, Some(ansi_256_to_rgb(0)));

        unsafe {
            env::set_var("COLORFGBG", "0;15");
        }
        let light = detect_from_colorfgbg_sample().unwrap();
        assert_eq!(light.mode, ThemeMode::Light);
        assert_eq!(light.background_rgb, Some(ansi_256_to_rgb(15)));

        unsafe {
            env::remove_var("COLORFGBG");
        }
    }

    #[test]
    fn theme_mode_from_rgb_maps_luminance() {
        assert_eq!(theme_mode_from_rgb(0, 0, 0), ThemeMode::Dark);
        assert_eq!(theme_mode_from_rgb(65535, 65535, 65535), ThemeMode::Light);
    }

    #[test]
    fn generated_default_toml_round_trips_app_defaults() {
        let default = AppConfig::default();
        let toml = build_default_config_toml().unwrap();
        let loaded = parse_app_config(&toml).unwrap();

        assert_eq!(
            loaded.to_persisted_config(),
            default.to_persisted_config(),
            "default config TOML should stay in sync with AppConfig defaults"
        );
    }

    #[test]
    fn rejects_legacy_color_fields() {
        let legacy = r#"
[theme]
mode = "light"
border = "yellow"
"#;

        assert!(parse_app_config(legacy).is_err());
    }
}
