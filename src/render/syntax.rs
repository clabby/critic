//! Fenced code highlighting via `tui-syntax-highlight` + `syntect`.

use crate::ui::{theme, theme::ThemeMode};
use ratatui::{
    style::Color,
    text::{Line, Span},
};
use syntect::{
    highlighting::ThemeSet,
    parsing::{SyntaxReference, SyntaxSet},
    util::LinesWithEndings,
};
use tui_syntax_highlight::Highlighter;

const OCEAN_DARK_THEME: &str = "base16-ocean.dark";
const OCEAN_LIGHT_THEME: &str = "base16-ocean.light";

/// Syntax highlighter for fenced markdown code blocks.
pub struct SyntaxHighlighter {
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
    theme_names: Vec<String>,
    active_theme_index: usize,
    highlighter: Highlighter,
}

impl Default for SyntaxHighlighter {
    fn default() -> Self {
        Self::new()
    }
}

impl SyntaxHighlighter {
    /// Initializes default syntect syntax/theme assets.
    pub fn new() -> Self {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let theme_set = ThemeSet::load_defaults();
        let mut theme_names: Vec<String> = theme_set.themes.keys().cloned().collect();
        theme_names.sort();

        let active_theme_index = select_theme_index(&theme_names);
        let highlighter = build_highlighter(&theme_set, &theme_names, active_theme_index);

        Self {
            syntax_set,
            theme_set,
            theme_names,
            active_theme_index,
            highlighter,
        }
    }

    /// Highlights a fenced code block using a best-effort language lookup.
    pub fn highlight(&self, lang: &str, source: &str) -> Vec<Line<'static>> {
        let syntax = resolve_syntax(&self.syntax_set, lang);
        self.highlight_with_syntax(source, syntax)
    }

    /// Highlights source and returns optional per-character foreground colors per line.
    pub fn highlight_foreground_for_path(
        &self,
        path: &str,
        source: &str,
    ) -> Vec<Vec<Option<Color>>> {
        let syntax = resolve_syntax_for_path(&self.syntax_set, path);
        match self.highlighter.highlight_lines(
            LinesWithEndings::from(source),
            syntax,
            &self.syntax_set,
        ) {
            Ok(text) => text
                .lines
                .into_iter()
                .map(|line| {
                    let mut colors = Vec::new();
                    for span in line.spans {
                        let fg = span.style.fg;
                        let width = span.content.chars().count();
                        colors.extend(std::iter::repeat_n(fg, width));
                    }
                    colors
                })
                .collect(),
            Err(_) => source
                .lines()
                .map(|line| vec![None; line.chars().count()])
                .collect(),
        }
    }

    pub fn set_theme(&mut self, name: &str) -> bool {
        let Some(index) = self
            .theme_names
            .iter()
            .position(|candidate| candidate.eq_ignore_ascii_case(name))
        else {
            return false;
        };

        self.active_theme_index = index;
        self.rebuild_highlighter();
        true
    }

    pub fn current_theme_name(&self) -> &str {
        self.theme_names
            .get(self.active_theme_index)
            .map(String::as_str)
            .unwrap_or("unknown")
    }

    pub fn set_ocean_theme(&mut self, mode: ThemeMode) -> bool {
        let name = match mode {
            ThemeMode::Dark => OCEAN_DARK_THEME,
            ThemeMode::Light => OCEAN_LIGHT_THEME,
        };
        self.set_theme(name)
    }

    fn rebuild_highlighter(&mut self) {
        self.highlighter =
            build_highlighter(&self.theme_set, &self.theme_names, self.active_theme_index);
    }

    fn highlight_with_syntax(&self, source: &str, syntax: &SyntaxReference) -> Vec<Line<'static>> {
        match self.highlighter.highlight_lines(
            LinesWithEndings::from(source),
            syntax,
            &self.syntax_set,
        ) {
            Ok(text) => text.lines,
            Err(_) => plain_code_lines(source),
        }
    }
}

fn select_theme_index(theme_names: &[String]) -> usize {
    const PREFERRED_THEME_NAMES: &[&str] = &["base16-ocean.dark", "base16-eighties.dark"];

    for preferred in PREFERRED_THEME_NAMES {
        if let Some(index) = theme_names
            .iter()
            .position(|name| name.eq_ignore_ascii_case(preferred))
        {
            return index;
        }
    }

    0
}

fn build_highlighter(
    theme_set: &ThemeSet,
    theme_names: &[String],
    active_theme_index: usize,
) -> Highlighter {
    let theme = theme_names
        .get(active_theme_index)
        .and_then(|name| theme_set.themes.get(name))
        .cloned()
        .unwrap_or_default();
    Highlighter::new(theme).line_numbers(false)
}

fn resolve_syntax<'a>(syntax_set: &'a SyntaxSet, lang: &str) -> &'a SyntaxReference {
    let normalized = normalize_lang(lang);
    syntax_set
        .find_syntax_by_token(&normalized)
        .or_else(|| syntax_set.find_syntax_by_name(&normalized))
        .unwrap_or_else(|| syntax_set.find_syntax_plain_text())
}

fn resolve_syntax_for_path<'a>(syntax_set: &'a SyntaxSet, path: &str) -> &'a SyntaxReference {
    if let Some(ext) = path.rsplit('.').next().filter(|value| *value != path)
        && let Some(syntax) = syntax_set.find_syntax_by_extension(ext)
    {
        return syntax;
    }

    syntax_set
        .find_syntax_by_name(path)
        .unwrap_or_else(|| syntax_set.find_syntax_plain_text())
}

fn normalize_lang(lang: &str) -> String {
    match lang.trim().to_lowercase().as_str() {
        "rs" => "rust".to_owned(),
        "js" => "javascript".to_owned(),
        "ts" => "typescript".to_owned(),
        "py" => "python".to_owned(),
        value => value.to_owned(),
    }
}

fn plain_code_lines(source: &str) -> Vec<Line<'static>> {
    source
        .lines()
        .map(|line| Line::from(vec![Span::styled(line.to_owned(), theme::text())]))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::SyntaxHighlighter;
    use crate::ui::theme::ThemeMode;

    #[test]
    fn includes_ocean_light_theme() {
        let highlighter = SyntaxHighlighter::new();
        assert!(
            highlighter
                .theme_names
                .iter()
                .any(|name| name.eq_ignore_ascii_case("base16-ocean.light")),
            "available themes: {:?}",
            highlighter.theme_names
        );
    }

    #[test]
    fn sets_ocean_theme_for_mode() {
        let mut highlighter = SyntaxHighlighter::new();
        assert!(highlighter.set_ocean_theme(ThemeMode::Light));
        assert_eq!(highlighter.current_theme_name(), "base16-ocean.light");
        assert!(highlighter.set_ocean_theme(ThemeMode::Dark));
        assert_eq!(highlighter.current_theme_name(), "base16-ocean.dark");
    }
}
