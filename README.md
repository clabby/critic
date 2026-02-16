# review-tui

Terminal UI for browsing GitHub pull request review threads.

## Features

- Authenticated GitHub client via `gh auth token` + `secrecy`.
- Search screen with fuzzy finding across open pull requests.
- Review screen with split panes:
  - Left: comment/thread navigator.
  - Right: rendered thread preview (markdown + treesitter-highlighted code blocks).
- Review mutations:
  - Reply to review threads.
  - Resolve/unresolve threads.
  - Submit pull request review as comment/approve/request-changes.
- Outdated comment indicator for threads without a valid source location.
- Operation spinner in the header with active async task label.
- Optional debug modes behind the `harness` cargo feature:
  - `--demo` deterministic fixture mode.
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

Demo mode (no network/API calls):

```bash
cargo run --features harness -- --demo
```

## Visual Harness

Render deterministic search/review frames to stdout (feature-gated):

```bash
cargo run --features harness -- --harness-dump --harness-width 140 --harness-height 44
```

This is useful for fast visual checks in CI or local iteration without opening the interactive TUI.

## Keybindings

### Search Screen

- `j`/`k` or arrow keys: move selection
- `type` + `Backspace`: fuzzy query
- `Enter`: open selected pull request
- `r`: refresh open pull request list
- `q`/`Esc`: quit

### Review Screen

- `j`/`k` or arrow keys: move selection
- `o` or `z`: collapse/expand selected thread root
- `t`: resolve/unresolve selected thread
- `f`: show/hide resolved threads
- `e`: edit pending reply for selected thread
- `s`: send pending reply
- `x`: clear pending reply
- `C`: open submit-review modal (comment)
- `A`: open submit-review modal (approve)
- `X`: open submit-review modal (request changes)
- `Enter`: submit active input modal
- `Esc`: close active input modal or go back to search
- `PageDown`/`PageUp`: scroll right preview pane
- `r`: refresh comments for current pull request
- `b`: back to search
- `q`: quit

Note: `--demo` and `--harness-dump` are only available when the `harness` feature is enabled.
In `--demo` mode, network mutations are intentionally disabled.

## Testing

```bash
cargo check
cargo test
```

## Module Layout

- `src/github/client.rs`: authenticated Octocrab client (`gh auth token`)
- `src/github/pulls.rs`: repository resolution + open PR fetching
- `src/github/comments.rs`: issue/review comments fetch + thread organization + resolution state
- `src/search/fuzzy.rs`: fuzzy ranking via `fuzzy-matcher`
- `src/render/markdown.rs`: markdown renderer
- `src/render/syntax.rs`: treesitter code block highlighting
- `src/ui/components/*`: shared/header components
- `src/ui/screens/search.rs`: PR search screen
- `src/ui/screens/review.rs`: split-pane review screen
- `src/harness/mod.rs`: deterministic frame dump harness
