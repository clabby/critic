//! External editor integration for composing replies/review messages.

use anyhow::{Context, Result, anyhow};
use crossterm::cursor::MoveTo;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use crossterm::terminal::{
    Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::env;
use std::fs;
use std::io::{Stdout, stdout};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

/// Opens an external editor and returns the edited contents.
///
/// Editor priority:
/// 1) `$VISUAL`
/// 2) `$EDITOR`
/// 3) `nvim`
/// 4) `vim`
/// 5) `vi`
pub fn edit_with_system_editor(
    initial_text: &str,
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
) -> Result<Option<String>> {
    let path = temp_file_path();
    fs::write(&path, initial_text)
        .with_context(|| format!("failed to write {}", path.display()))?;

    suspend_tui()?;

    let edit_result = run_editor(&path);

    let resume_result = resume_tui(terminal);

    let output = match edit_result {
        Ok(()) => {
            let text = fs::read_to_string(&path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            Ok(Some(text))
        }
        Err(err) => Err(err),
    };

    let _ = fs::remove_file(&path);

    if let Err(err) = resume_result {
        return Err(err.context("failed to restore TUI after editor"));
    }

    output
}

/// Opens a file path in the user's preferred editor.
pub fn edit_file_with_system_editor(path: &Path) -> Result<()> {
    run_editor(path)
}

fn run_editor(path: &Path) -> Result<()> {
    let mut candidates = Vec::new();

    if let Some(visual) = env::var_os("VISUAL") {
        let visual = visual.to_string_lossy().trim().to_owned();
        if !visual.is_empty() {
            candidates.push(visual);
        }
    }

    if let Some(editor) = env::var_os("EDITOR") {
        let editor = editor.to_string_lossy().trim().to_owned();
        if !editor.is_empty() {
            candidates.push(editor);
        }
    }

    candidates.extend(["nvim".to_owned(), "vim".to_owned(), "vi".to_owned()]);

    for command in candidates {
        let mut parts = command.split_whitespace();
        let Some(program) = parts.next() else {
            continue;
        };
        let args: Vec<String> = parts.map(|part| part.to_owned()).collect();

        let status = Command::new(program).args(&args).arg(path).status();
        match status {
            Ok(status) => {
                if status.success() {
                    return Ok(());
                }
                return Err(anyhow!(
                    "editor `{}` exited with status {}",
                    command,
                    status
                        .code()
                        .map(|code| code.to_string())
                        .unwrap_or_else(|| "unknown".to_owned())
                ));
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                continue;
            }
            Err(err) => {
                return Err(anyhow!("failed to launch editor `{}`: {}", command, err));
            }
        }
    }

    Err(anyhow!(
        "no editor found (tried $VISUAL, $EDITOR, nvim, vim, vi)"
    ))
}

fn temp_file_path() -> PathBuf {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);

    env::temp_dir().join(format!("critic-edit-{}-{}.md", std::process::id(), millis))
}

fn suspend_tui() -> Result<()> {
    execute!(stdout(), LeaveAlternateScreen, DisableMouseCapture)
        .context("failed to release terminal for external editor")?;
    disable_raw_mode().context("failed to disable raw mode")?;
    Ok(())
}

fn resume_tui(terminal: &mut Terminal<CrosstermBackend<Stdout>>) -> Result<()> {
    execute!(
        stdout(),
        EnterAlternateScreen,
        EnableMouseCapture,
        Clear(ClearType::All),
        MoveTo(0, 0)
    )
    .context("failed to restore terminal view")?;
    enable_raw_mode().context("failed to enable raw mode")?;
    terminal
        .clear()
        .context("failed to clear terminal buffer")?;
    Ok(())
}
