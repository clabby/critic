//! Markdown rendering for the right pane preview.

use crate::domain::PullRequestDiffFile;
use crate::render::syntax::SyntaxHighlighter;
use crate::ui::theme;
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use std::collections::HashMap;

type SyntaxColorLine = Vec<Option<Color>>;

#[derive(Debug, Clone)]
struct ListState {
    ordered: bool,
    next_index: u64,
}

#[derive(Debug, Clone)]
struct CodeBlockState {
    language: String,
    content: String,
}

/// Markdown renderer with syntect-based fenced code highlighting.
#[derive(Default)]
pub struct MarkdownRenderer {
    syntax: SyntaxHighlighter,
    diff_cache: HashMap<DiffCacheKey, DiffFileHighlights>,
}

impl MarkdownRenderer {
    pub fn new() -> Self {
        Self {
            syntax: SyntaxHighlighter::new(),
            diff_cache: HashMap::new(),
        }
    }

    pub fn clear_diff_cache(&mut self) {
        self.diff_cache.clear();
    }

    pub fn set_syntax_theme(&mut self, name: &str) -> bool {
        let changed = self.syntax.set_theme(name);
        if changed {
            self.diff_cache.clear();
        }
        changed
    }

    pub fn cycle_syntax_theme(&mut self) -> &str {
        let name = self.syntax.cycle_theme();
        self.diff_cache.clear();
        name
    }

    pub fn current_syntax_theme_name(&self) -> &str {
        self.syntax.current_theme_name()
    }

    pub fn diff_file_highlights(
        &mut self,
        file: &PullRequestDiffFile,
    ) -> (&[SyntaxColorLine], &[SyntaxColorLine]) {
        let key = DiffCacheKey {
            theme: self.current_syntax_theme_name().to_owned(),
            path: file.path.clone(),
        };

        let entry = self.diff_cache.entry(key).or_insert_with(|| {
            let left_source = build_diff_side_source(file, DiffSide::Left);
            let right_source = build_diff_side_source(file, DiffSide::Right);

            DiffFileHighlights {
                left: self
                    .syntax
                    .highlight_foreground_for_path(&file.path, &left_source),
                right: self
                    .syntax
                    .highlight_foreground_for_path(&file.path, &right_source),
            }
        });

        (&entry.left, &entry.right)
    }

    /// Renders markdown into styled ratatui lines.
    pub fn render(&mut self, text: &str) -> Vec<Line<'static>> {
        let parser = Parser::new_ext(text, Options::all());

        let mut lines: Vec<Vec<Span<'static>>> = vec![Vec::new()];
        let mut style_stack: Vec<Style> = vec![theme::text()];
        let mut list_stack: Vec<ListState> = Vec::new();
        let mut in_code_block: Option<CodeBlockState> = None;

        for event in parser {
            if let Some(code) = in_code_block.as_mut() {
                match event {
                    Event::Text(content) | Event::Code(content) => code.content.push_str(&content),
                    Event::SoftBreak | Event::HardBreak => code.content.push('\n'),
                    Event::End(TagEnd::CodeBlock) => {
                        let highlighted = self.syntax.highlight(&code.language, &code.content);
                        for mut line in highlighted {
                            let mut prefixed = vec![Span::styled("  ", theme::dim())];
                            prefixed.append(&mut line.spans);
                            lines.push(prefixed);
                        }
                        lines.push(Vec::new());
                        in_code_block = None;
                    }
                    _ => {}
                }
                continue;
            }

            match event {
                Event::Start(tag) => match tag {
                    Tag::Paragraph => {}
                    Tag::Heading { .. } => {
                        push_nonempty_newline(&mut lines);
                        let base = *style_stack.last().unwrap_or(&Style::default());
                        style_stack.push(base.add_modifier(Modifier::BOLD));
                    }
                    Tag::Strong => {
                        let base = *style_stack.last().unwrap_or(&Style::default());
                        style_stack.push(base.add_modifier(Modifier::BOLD));
                    }
                    Tag::Emphasis => {
                        let base = *style_stack.last().unwrap_or(&Style::default());
                        style_stack.push(base.add_modifier(Modifier::ITALIC));
                    }
                    Tag::Strikethrough => {
                        let base = *style_stack.last().unwrap_or(&Style::default());
                        style_stack.push(base.add_modifier(Modifier::CROSSED_OUT));
                    }
                    Tag::List(start) => {
                        let ordered = start.is_some();
                        let next_index = start.unwrap_or(1);
                        list_stack.push(ListState {
                            ordered,
                            next_index,
                        });
                    }
                    Tag::Item => {
                        push_nonempty_newline(&mut lines);
                        let depth = list_stack.len().saturating_sub(1);
                        let indent = "  ".repeat(depth);
                        let marker = match list_stack.last_mut() {
                            Some(state) if state.ordered => {
                                let current = state.next_index;
                                state.next_index += 1;
                                format!("{indent}{current}. ")
                            }
                            _ => format!("{indent}- "),
                        };
                        push_span(&mut lines, Span::styled(marker, theme::dim()));
                    }
                    Tag::CodeBlock(kind) => {
                        push_nonempty_newline(&mut lines);
                        let language = match kind {
                            CodeBlockKind::Fenced(lang) => lang.to_string(),
                            CodeBlockKind::Indented => "text".to_owned(),
                        };
                        in_code_block = Some(CodeBlockState {
                            language,
                            content: String::new(),
                        });
                    }
                    Tag::Link { .. } => {
                        let base = *style_stack.last().unwrap_or(&Style::default());
                        style_stack.push(
                            base.add_modifier(Modifier::UNDERLINED)
                                .fg(theme::link_color()),
                        );
                    }
                    _ => {}
                },
                Event::End(tag_end) => match tag_end {
                    TagEnd::Paragraph => {
                        push_nonempty_newline(&mut lines);
                        lines.push(Vec::new());
                    }
                    TagEnd::Heading(_) => {
                        pop_style_if_possible(&mut style_stack);
                        push_nonempty_newline(&mut lines);
                        lines.push(Vec::new());
                    }
                    TagEnd::Strong | TagEnd::Emphasis | TagEnd::Strikethrough | TagEnd::Link => {
                        pop_style_if_possible(&mut style_stack);
                    }
                    TagEnd::Item => {
                        push_nonempty_newline(&mut lines);
                    }
                    TagEnd::List(_) => {
                        list_stack.pop();
                        push_nonempty_newline(&mut lines);
                    }
                    TagEnd::CodeBlock => {}
                    _ => {}
                },
                Event::Text(content) => {
                    let style = *style_stack.last().unwrap_or(&Style::default());
                    push_span(&mut lines, Span::styled(content.to_string(), style));
                }
                Event::Code(content) => {
                    push_span(
                        &mut lines,
                        Span::styled(format!("`{content}`"), theme::inline_code()),
                    );
                }
                Event::SoftBreak | Event::HardBreak => {
                    push_nonempty_newline(&mut lines);
                }
                Event::Rule => {
                    push_nonempty_newline(&mut lines);
                    push_span(
                        &mut lines,
                        Span::styled("────────────────────────────────────────", theme::dim()),
                    );
                    push_nonempty_newline(&mut lines);
                }
                _ => {}
            }
        }

        while lines.last().is_some_and(|line| line.is_empty()) && lines.len() > 1 {
            lines.pop();
        }

        lines.into_iter().map(Line::from).collect()
    }
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
struct DiffCacheKey {
    theme: String,
    path: String,
}

#[derive(Debug, Clone, Default)]
struct DiffFileHighlights {
    left: Vec<SyntaxColorLine>,
    right: Vec<SyntaxColorLine>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum DiffSide {
    Left,
    Right,
}

fn build_diff_side_source(file: &PullRequestDiffFile, side: DiffSide) -> String {
    let mut max_line = 0usize;
    for row in &file.rows {
        let line = match side {
            DiffSide::Left => row.left_line_number,
            DiffSide::Right => row.right_line_number,
        };
        max_line = max_line.max(line.unwrap_or(0));
    }

    if max_line == 0 {
        return String::new();
    }

    let mut lines = vec![String::new(); max_line];
    for row in &file.rows {
        match side {
            DiffSide::Left => {
                if let Some(line) = row.left_line_number {
                    lines[line.saturating_sub(1)] = row.left_text.clone();
                }
            }
            DiffSide::Right => {
                if let Some(line) = row.right_line_number {
                    lines[line.saturating_sub(1)] = row.right_text.clone();
                }
            }
        }
    }

    let mut source = lines.join("\n");
    source.push('\n');
    source
}

fn push_span(lines: &mut Vec<Vec<Span<'static>>>, span: Span<'static>) {
    if let Some(line) = lines.last_mut() {
        line.push(span);
    }
}

fn push_nonempty_newline(lines: &mut Vec<Vec<Span<'static>>>) {
    if lines.last().is_some_and(|line| line.is_empty()) {
        return;
    }
    lines.push(Vec::new());
}

fn pop_style_if_possible(style_stack: &mut Vec<Style>) {
    if style_stack.len() > 1 {
        style_stack.pop();
    }
}
