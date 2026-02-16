use clap::Parser;
use review_tui::app::{self, AppConfig};
#[cfg(feature = "harness")]
use review_tui::harness;

/// Terminal UI for GitHub pull-request review thread browsing.
#[derive(Debug, Parser)]
#[command(version, about)]
struct Cli {
    /// Repository owner. If omitted with `--repo`, both are resolved via `gh repo view`.
    #[arg(long, requires = "repo")]
    owner: Option<String>,

    /// Repository name. If omitted with `--owner`, both are resolved via `gh repo view`.
    #[arg(long, requires = "owner")]
    repo: Option<String>,

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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    #[cfg(feature = "harness")]
    if cli.harness_dump {
        let dump = harness::render_demo_dump(cli.harness_width, cli.harness_height)?;
        println!("{dump}");
        return Ok(());
    }

    app::run(AppConfig {
        owner: cli.owner,
        repo: cli.repo,
    })
    .await
}
