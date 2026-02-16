//! Treesitter-backed fenced code highlighting for the right pane.

use anyhow::Result;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use std::collections::HashMap;
use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};

const HIGHLIGHT_NAMES: &[&str] = &[
    "attribute",
    "comment",
    "constant",
    "constant.builtin",
    "constructor",
    "function",
    "function.builtin",
    "keyword",
    "number",
    "operator",
    "property",
    "punctuation",
    "string",
    "tag",
    "type",
    "type.builtin",
    "variable",
    "variable.builtin",
];

/// Syntax highlighter for fenced markdown code blocks.
pub struct SyntaxHighlighter {
    configs: HashMap<String, HighlightConfiguration>,
}

impl Default for SyntaxHighlighter {
    fn default() -> Self {
        Self::new()
    }
}

impl SyntaxHighlighter {
    /// Initializes known treesitter language configurations.
    pub fn new() -> Self {
        let mut configs = HashMap::new();

        insert_config(
            &mut configs,
            "rust",
            tree_sitter_rust::LANGUAGE.into(),
            tree_sitter_rust::HIGHLIGHTS_QUERY,
            "",
            "",
        );

        insert_config(
            &mut configs,
            "go",
            tree_sitter_go::LANGUAGE.into(),
            tree_sitter_go::HIGHLIGHTS_QUERY,
            "",
            "",
        );

        insert_config(
            &mut configs,
            "javascript",
            tree_sitter_javascript::LANGUAGE.into(),
            tree_sitter_javascript::HIGHLIGHT_QUERY,
            "",
            "",
        );

        insert_config(
            &mut configs,
            "typescript",
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            tree_sitter_typescript::HIGHLIGHTS_QUERY,
            "",
            "",
        );

        insert_config(
            &mut configs,
            "python",
            tree_sitter_python::LANGUAGE.into(),
            tree_sitter_python::HIGHLIGHTS_QUERY,
            "",
            "",
        );

        Self { configs }
    }

    /// Highlights a fenced code block with treesitter when the language is supported.
    pub fn highlight(&mut self, lang: &str, source: &str) -> Vec<Line<'static>> {
        let normalized = normalize_lang(lang);
        let Some(config) = self.configs.get(&normalized) else {
            return plain_code_lines(source);
        };

        match self.highlight_with_config(config, source) {
            Ok(lines) if !lines.is_empty() => lines,
            _ => plain_code_lines(source),
        }
    }

    fn highlight_with_config(
        &self,
        config: &HighlightConfiguration,
        source: &str,
    ) -> Result<Vec<Line<'static>>> {
        let mut highlighter = Highlighter::new();
        let events = highlighter.highlight(config, source.as_bytes(), None, |_| None)?;

        let mut lines: Vec<Vec<Span<'static>>> = vec![Vec::new()];
        let mut style_stack: Vec<Style> = vec![base_code_style()];

        for event in events {
            match event? {
                HighlightEvent::HighlightStart(group) => {
                    let style = style_for_capture(group.0);
                    style_stack.push(style);
                }
                HighlightEvent::HighlightEnd => {
                    if style_stack.len() > 1 {
                        style_stack.pop();
                    }
                }
                HighlightEvent::Source { start, end } => {
                    let text = &source[start..end];
                    let style = *style_stack.last().unwrap_or(&base_code_style());
                    push_text_with_style(&mut lines, text, style);
                }
            }
        }

        Ok(lines.into_iter().map(Line::from).collect())
    }
}

fn insert_config(
    configs: &mut HashMap<String, HighlightConfiguration>,
    key: &str,
    language: tree_sitter::Language,
    highlights: &str,
    injections: &str,
    locals: &str,
) {
    let Ok(mut config) = HighlightConfiguration::new(language, "", highlights, injections, locals)
    else {
        return;
    };
    config.configure(HIGHLIGHT_NAMES);
    configs.insert(key.to_owned(), config);
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

fn style_for_capture(index: usize) -> Style {
    let name = HIGHLIGHT_NAMES.get(index).copied().unwrap_or("");

    if name.contains("comment") {
        Style::default().fg(Color::Gray)
    } else if name.contains("string") {
        Style::default().fg(Color::Green)
    } else if name.contains("keyword") {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else if name.contains("number") || name.contains("constant") {
        Style::default().fg(Color::Cyan)
    } else if name.contains("function") {
        Style::default().fg(Color::LightBlue)
    } else if name.contains("type") {
        Style::default().fg(Color::Magenta)
    } else {
        base_code_style()
    }
}

fn base_code_style() -> Style {
    Style::default().fg(Color::Rgb(210, 210, 200))
}

fn push_text_with_style(lines: &mut Vec<Vec<Span<'static>>>, text: &str, style: Style) {
    for (index, segment) in text.split('\n').enumerate() {
        if index > 0 {
            lines.push(Vec::new());
        }

        if !segment.is_empty() {
            if let Some(last) = lines.last_mut() {
                last.push(Span::styled(segment.to_owned(), style));
            }
        }
    }
}

fn plain_code_lines(source: &str) -> Vec<Line<'static>> {
    source
        .lines()
        .map(|line| Line::from(vec![Span::styled(line.to_owned(), base_code_style())]))
        .collect()
}
