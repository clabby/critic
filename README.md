# review-tui

Terminal UI for browsing GitHub pull request review threads.

## Features

- Authenticated GitHub client via `gh auth token` + `secrecy`.
- Search screen with fuzzy finding across open pull requests.
- Search screen review badges: `✅` approved, `❌` changes requested.
- Review screen with split panes:
  - Left: comment/thread navigator.
  - Right: rendered thread preview (markdown + syntax-highlighted code blocks via `tui-syntax-highlight`).
- Review mutations:
  - Reply to review threads.
  - Resolve/unresolve threads.
  - Submit pull request review as comment/approve/request-changes.
- Outdated comment indicator for threads without a valid source location.
- Operation spinner in the header with active async task label.
- Optional debug modes behind the `harness` cargo feature:
  - `--harness-dump` non-interactive frame dump mode.

## Requirements

- Rust toolchain
- GitHub CLI authenticated (`gh auth login`)

## Run

```bash
cargo run
```

Optional explicit repository:

```bash
cargo run -- --owner <owner> --repo <repo>
```

Config utilities:

```bash
cargo run -- config --path
cargo run -- config --edit
```

## Configuration

On startup, `review-tui` loads `~/.review-tui/config.toml`. If the file does not exist, it is created automatically with defaults.

Theme colors are configurable under `[theme]`:

```toml
[theme]
border = "#c47832"
title = "#ebaa5a"
dim = "dark_gray"
text = "#d2d2c8"
selected_fg = "black"
selected_bg = "#e2b45c"
```

Supported color formats:
- `#RRGGBB`
- named ANSI colors like `black`, `yellow`, `light_blue`, `dark_gray`

## Visual Harness

Render deterministic search/review frames to stdout (feature-gated):

```bash
cargo run --features harness -- --harness-dump --harness-width 140 --harness-height 44
```

This is useful for fast visual checks in CI or local iteration without opening the interactive TUI.

## Keybindings

### Search Screen

- `j`/`k` or arrow keys: move selection
- `s`: focus search input
- `Enter`/`Esc`: unfocus search input
- `type` + `Backspace`: edit fuzzy query (while focused)
- `Enter`: open selected pull request
- `R`: refresh open pull request list
- `q`: quit

### Review Screen

- `j`/`k` or arrow keys: move selection
- `o` or `z`: collapse/expand selected thread root
- `t`: resolve/unresolve selected thread
- `f`: show/hide resolved threads
- `e`: edit pending reply for selected thread
- `s`: send staged pending reply
- `x`: clear pending reply
- `C`: open editor and submit review comment
- `A`: open editor and submit approval
- `X`: open editor and submit request changes
- `Esc`: back to search
- `PageDown`/`PageUp`: scroll right preview pane
- `R`: refresh comments for current pull request
- `b`: back to search
- `q`: quit

Note: `--harness-dump` is only available when the `harness` feature is enabled.
Interactive compose uses external editor fallback order: `$VISUAL`, `$EDITOR`, `nvim`, `vim`, `vi`.

## Testing

```bash
cargo check
cargo test
```

## Module Layout

- `src/github/client.rs`: authenticated Octocrab client (`gh auth token`)
- `src/config.rs`: config loader + default `~/.review-tui/config.toml` bootstrap
- `src/github/pulls.rs`: repository resolution + open PR fetching
- `src/github/comments.rs`: issue/review comments fetch + thread organization + resolution state
- `src/search/fuzzy.rs`: fuzzy ranking via `fuzzy-matcher`
- `src/render/markdown.rs`: markdown renderer
- `src/render/syntax.rs`: `tui-syntax-highlight` code block highlighting
- `src/ui/components/*`: shared/header components
- `src/ui/screens/search.rs`: PR search screen
- `src/ui/screens/review.rs`: split-pane review screen
- `src/harness/mod.rs`: deterministic frame dump harness
- `src/harness/fixtures.rs`: harness-only deterministic fixture data
