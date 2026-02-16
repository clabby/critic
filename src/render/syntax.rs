//! Fenced code highlighting via `tui-syntax-highlight` + `syntect`.

use crate::ui::theme;
use ratatui::text::{Line, Span};
use syntect::highlighting::{Theme, ThemeSet};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use syntect::util::LinesWithEndings;
use tui_syntax_highlight::Highlighter;

/// Syntax highlighter for fenced markdown code blocks.
#[derive(Clone)]
pub struct SyntaxHighlighter {
    syntax_set: SyntaxSet,
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
        let theme = select_theme(&theme_set);
        let highlighter = Highlighter::new(theme).line_numbers(false);

        Self {
            syntax_set,
            highlighter,
        }
    }

    /// Highlights a fenced code block using a best-effort language lookup.
    pub fn highlight(&self, lang: &str, source: &str) -> Vec<Line<'static>> {
        let syntax = resolve_syntax(&self.syntax_set, lang);

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

fn select_theme(theme_set: &ThemeSet) -> Theme {
    const PREFERRED_THEME_NAMES: &[&str] = &["base16-ocean.dark", "base16-eighties.dark"];

    for name in PREFERRED_THEME_NAMES {
        if let Some(theme) = theme_set.themes.get(*name) {
            return theme.clone();
        }
    }

    theme_set
        .themes
        .values()
        .next()
        .cloned()
        .unwrap_or_default()
}

fn resolve_syntax<'a>(syntax_set: &'a SyntaxSet, lang: &str) -> &'a SyntaxReference {
    let normalized = normalize_lang(lang);
    syntax_set
        .find_syntax_by_token(&normalized)
        .or_else(|| syntax_set.find_syntax_by_name(&normalized))
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
