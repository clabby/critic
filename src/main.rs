use clap::{ArgGroup, Args, Parser, Subcommand};
use critic::app::editor;
use critic::app::{self, AppConfig};
use critic::config;
#[cfg(feature = "harness")]
use critic::harness;
use critic::ui::theme;

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

    #[cfg(feature = "harness")]
    /// Render deterministic frames to stdout without entering interactive mode.
    #[arg(long, default_value_t = false)]
    harness_dump: bool,

    #[cfg(feature = "harness")]
    /// Harness frame width.
    #[arg(long, default_value_t = 140)]
    harness_width: u16,

    #[cfg(feature = "harness")]
    /// Harness frame height.
    #[arg(long, default_value_t = 44)]
    harness_height: u16,
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
    theme::apply(config.theme);

    #[cfg(feature = "harness")]
    if cli.harness_dump {
        let dump = harness::render_demo_dump(cli.harness_width, cli.harness_height)?;
        println!("{dump}");
        return Ok(());
    }

    app::run(AppConfig {
        owner: cli.owner,
        repo: cli.repo,
        pull: cli.pull,
        syntax_theme: config.syntax_theme,
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
