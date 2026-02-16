//! Visual harness for deterministic rendering snapshots.

use crate::app::state::AppState;
use crate::domain::Route;
use crate::fixtures;
use crate::render::markdown::MarkdownRenderer;
use crate::ui;
use anyhow::Context;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

/// Renders demo search and review screens into plain text.
pub fn render_demo_dump(width: u16, height: u16) -> anyhow::Result<String> {
    let search = render_demo_search(width, height)?;
    let review = render_demo_review(width, height)?;

    Ok(format!(
        "=== SEARCH SCREEN ===\n{search}\n\n=== REVIEW SCREEN ===\n{review}\n"
    ))
}

fn render_demo_search(width: u16, height: u16) -> anyhow::Result<String> {
    let mut state = AppState::default();
    let pulls = fixtures::demo_pull_requests();
    let label = pulls
        .first()
        .map(|pull| format!("{}/{}", pull.owner, pull.repo))
        .unwrap_or_else(|| "demo/repository".to_owned());

    state.set_repository_label(label);
    state.set_pull_requests(pulls);
    render_state_to_string(&state, width, height)
}

fn render_demo_review(width: u16, height: u16) -> anyhow::Result<String> {
    let mut state = AppState::default();
    let pulls = fixtures::demo_pull_requests();
    let pull = pulls
        .first()
        .cloned()
        .context("missing demo pull request")?;

    state.set_repository_label(format!("{}/{}", pull.owner, pull.repo));
    state.set_pull_requests(pulls);
    state.open_review(pull.clone(), fixtures::demo_pull_request_data_for(&pull));
    state.route = Route::Review;
    render_state_to_string(&state, width, height)
}

fn render_state_to_string(state: &AppState, width: u16, height: u16) -> anyhow::Result<String> {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).context("failed to create test terminal")?;
    let mut markdown = MarkdownRenderer::new();

    terminal
        .draw(|frame| ui::render(frame, state, &mut markdown))
        .context("failed to render frame")?;

    let buffer = terminal.backend().buffer().clone();

    let mut out = String::new();
    for y in 0..height {
        for x in 0..width {
            out.push_str(buffer[(x, y)].symbol());
        }
        while out.ends_with(' ') {
            out.pop();
        }
        out.push('\n');
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::render_demo_dump;

    #[test]
    fn demo_dump_contains_both_screens() {
        let dump = render_demo_dump(120, 36).expect("render should succeed");
        assert!(dump.contains("=== SEARCH SCREEN ==="));
        assert!(dump.contains("=== REVIEW SCREEN ==="));
        assert!(dump.contains("review-tui"));
    }
}
