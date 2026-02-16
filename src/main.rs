#![doc = include_str!("../README.md")]

mod app;
mod config;
mod domain;
mod github;
mod render;
mod search;
mod ui;

use crate::{
    app::{AppConfig, editor},
    ui::theme,
};
use clap::{ArgGroup, Args, Parser, Subcommand};

/// Terminal UI for GitHub pull-request review thread browsing.
#[derive(Debug, Parser)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// Repository owner. If omitted with `--repo`, both are resolved via `gh repo view`.
    #[arg(long, requires = "repo")]
    owner: Option<String>,

    /// Repository name. If omitted with `--owner`, both are resolved via `gh repo view`.
    #[arg(long, requires = "owner")]
    repo: Option<String>,

    /// Pull request number to open directly on startup.
    #[arg(long)]
    pull: Option<u64>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Inspect or edit critic configuration.
    Config(ConfigCommand),
}

#[derive(Debug, Args)]
#[command(group(
    ArgGroup::new("config_action")
        .required(true)
        .multiple(false)
        .args(["edit", "path"])
))]
struct ConfigCommand {
    /// Open the config file in $VISUAL/$EDITOR/nvim/vim/vi.
    #[arg(long)]
    edit: bool,

    /// Print the config file path.
    #[arg(long)]
    path: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if let Some(Command::Config(command)) = cli.command {
        return handle_config_command(command);
    }

    let config = config::load_or_create()?;
    let runtime_theme = config.resolve_runtime_theme();
    let terminal_background = config::detect_terminal_background_rgb();
    theme::apply(
        runtime_theme.palette,
        runtime_theme.mode,
        terminal_background,
    );

    crate::app::run(AppConfig {
        owner: cli.owner,
        repo: cli.repo,
        pull: cli.pull,
        initial_theme_mode: runtime_theme.mode,
        initial_terminal_background: terminal_background,
        theme_config: config,
    })
    .await
}

fn handle_config_command(command: ConfigCommand) -> anyhow::Result<()> {
    let path = config::ensure_config_file()?;

    if command.path {
        println!("{}", path.display());
        return Ok(());
    }

    if command.edit {
        editor::edit_file_with_system_editor(path.as_path())?;
        return Ok(());
    }

    Ok(())
}
