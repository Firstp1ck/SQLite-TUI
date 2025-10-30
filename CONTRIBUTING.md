# Contributing to sqlite-editor

Thanks for your interest in contributing! This document outlines how to build and run the project, how to file issues and submit pull requests, the coding standards we follow, our commit style guidelines, and our Developer Certificate of Origin (DCO) policy.

By participating, you agree to abide by our Code of Conduct (CODE_OF_CONDUCT.md) and our Security Policy (SECURITY.md).

## Ways to contribute

- Report bugs and request features via Issues
- Improve documentation
- Fix bugs and implement features
- Refactor and improve code quality and test coverage

For large or controversial changes, please open an Issue first to discuss the approach.

---

## Build and run

Prerequisites:
- Rust stable toolchain (https://rustup.rs)

Build:
- Debug: `cargo build`
- Release: `cargo build --release`

Run:
- `cargo run -- /path/to/database.db`
- With page size: `cargo run -- -n 500 /path/to/database.db`

Notes:
- The project uses `rusqlite`. Depending on configuration, it may build SQLite itself (“bundled” feature) or link to your system’s SQLite. If your configuration requires it, ensure `libsqlite3` development headers are available on your system.

Recommended local checks before opening a PR:
- Format: `cargo fmt --all`
- Lints: `cargo clippy --workspace --all-targets -- -D warnings`
- Build: `cargo build` (or `cargo build --release`)

---

## Project layout (high level)

- `src/main.rs` — CLI, terminal setup, event loop
- `src/app.rs` — application state and input handling
- `src/ui.rs` — ratatui layout and rendering
- `src/db.rs` — database access and worker thread

This TUI uses `ratatui` for rendering, `crossterm` for input/terminal control, and a background DB worker (via channels) to keep the UI responsive.

---

## Submitting Issues

Before filing a new issue:
- Search existing issues to avoid duplicates
- Include environment details (OS, terminal, Rust version)
- Provide steps to reproduce, expected vs. actual behavior, logs/screenshots if relevant
- For security concerns, please follow SECURITY.md and use private disclosure

Issue templates are available in the repository to guide you.

---

## Submitting Pull Requests

General guidelines:
- Keep PRs focused and small when possible
- Write clear descriptions: what, why, how
- Link related issues (e.g., “Fixes #123”)
- Include tests where practical and update documentation when behavior changes
- Ensure CI (if configured) and local checks pass:
  - `cargo fmt --all`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo build`

Process:
1. Fork the repo and create a feature branch (`feat/my-change`)
2. Commit using the style below (with DCO sign-off)
3. Push and open a PR
4. Address review comments; we prefer amending or rebasing to keep history clean
5. A maintainer will merge once approved

Merge strategy:
- We generally prefer squash-and-merge for clarity unless a series of commits is intentionally structured

---

## Coding standards

- Edition: Rust 2024
- Formatting: `rustfmt` (use `cargo fmt --all`)
- Linting: `clippy` with warnings treated as errors for the workspace
- Error handling:
  - Use `Result` and propagate errors with `?`
  - Prefer `anyhow::Result` in application layers; use specific error types in libraries if introduced
  - Avoid `unwrap`/`expect` in non-test code unless truly impossible to fail; add context if needed
- Logging/diagnostics:
  - Prefer actionable error messages surfaced to users
- Performance:
  - Keep UI responsive; offload long operations to the DB worker
  - Be cautious with full-table scans and `COUNT(*)` on large tables
- UI:
  - Avoid panics in the draw loop
  - Keep keybindings consistent and discoverable; document changes in README

---

## Commit style

We recommend Conventional Commits. At minimum, use an imperative, present-tense summary.

Examples:
- `feat(ui): add inline cell editing`
- `fix(db): handle NULL correctly when updating`
- `docs: document key bindings and usage`
- `refactor: extract pagination logic`
- `chore: run cargo fmt`

Guidelines:
- Summary line ≤ 72 chars
- Use body for motivation, context, and breaking changes
- Reference issues when applicable (e.g., `Fixes #123`)

---

## Developer Certificate of Origin (DCO)

We require a DCO sign-off on all commits. By signing off your commits, you certify that you have the right to submit the work under the project’s license.

Sign-off line (must be in every commit message):
```
Signed-off-by: Your Name <your.email@example.com>
```

How to sign off automatically:
- Command line: `git commit -s -m "feat: your message"`
- If using GitHub’s web UI, you can append the sign-off line manually to the commit message

If you forgot to sign a commit:
- Amend the last commit: `git commit --amend -s`
- Or re-sign a range: `git rebase --rebase-merges --signoff <base>` (or `git rebase -i --signoff <base>`)

PRs without DCO may be asked to amend before merging.

---

## License

By contributing, you agree that your contributions will be licensed under the repository’s LICENSE.

Thank you for contributing to sqlite-editor!
