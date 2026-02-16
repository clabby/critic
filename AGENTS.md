# AGENTS.md

This file defines engineering practices for contributors and coding agents working on `critic`.

## Core principles

- Keep behavior simple and predictable.
- Keep modules focused and loosely coupled.
- Surface clear, actionable errors to users.
- Prioritize user trust: safe defaults, explicit state transitions, and no silent side effects.
- Favor straightforward UX over clever interactions.

## Code organization

- Keep concerns separated:
  - `src/github/*`: API/auth/mutations.
  - `src/app/*`: event loop, state machine, async orchestration.
  - `src/ui/*`: rendering and interaction views.
  - `src/render/*`: markdown/syntax rendering.
  - `src/search/*`: fuzzy matching.
- Avoid large monolithic modules when behavior can be isolated.
- Keep data models in `src/domain/*` stable and explicit.

## Dependency-first policy

- Prefer existing dependency types and components before building custom equivalents.
- Re-use `octocrab` models for GitHub API payloads whenever possible instead of defining mirror structs.
- Re-use `ratatui` widgets/components for layout, scrolling, gauges, and related UI behavior before custom drawing.
- Add custom wrappers only when a dependency does not expose what we need, and keep that wrapper minimal.
- Treat custom replacements as a last resort because they increase fragility and maintenance burden.

## Async and UX rules

- Never block the UI loop on network work.
- All GitHub mutations must refresh pull request data before UI settles.
- Show operation state in the header spinner with descriptive labels.
- On failure, preserve context and show a concise error message.

## Error handling

- Do not swallow errors.
- Return typed errors from lower layers (`thiserror`) and user-facing context at boundaries.
- Prefer specific messages (which operation failed and why) over generic failures.

## Debugging workflow

### Fast compile/test cycle

- `cargo check`
- `cargo test`

### Interactive verification

- Live mode: `cargo run`

## Change discipline

- Keep diffs focused and minimal.
- Update `README.md` when user-visible behavior or flags change.
- Add or update tests for parser/state/aggregation logic when behavior changes.
