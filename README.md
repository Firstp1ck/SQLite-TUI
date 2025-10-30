# Sqlite‑TUI

[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)
[![Made with Rust](https://img.shields.io/badge/Made%20with-Rust-orange.svg)](https://www.rust-lang.org/)

Sqlite‑TUI (binary: `sqlite-editor`) is a fast, keyboard‑first terminal UI for browsing and editing SQLite databases — smooth scrolling, inline edits, and streamlined workflows.

<p align="center">
✨ Idea or bug? <strong><a href="https://github.com/Firstp1ck/Sqlite-TUI/issues">Open an issue</a></strong>.<br/>
❤️ Thank you to the community for your ideas, reports, and support!
</p>

## Table of Contents
- [Quick start](#quick-start)
- [Features](#features)
- [Usage](#usage)
  - [Handy shortcuts](#handy-shortcuts)
- [Troubleshooting](#troubleshooting)
- [Roadmap](#roadmap)
- [Credits](#credits)
- [License](#license)

## Quick start
- Install (from source, debug):
```bash
git clone https://github.com/Firstp1ck/Sqlite-TUI
cd Sqlite-TUI
cargo run -- ./path/to/your.db
```

- Build (release) and run:
```bash
cargo build --release
./target/release/sqlite-editor ./path/to/your.db
```

- Optional: install locally
```bash
cargo install --path .
sqlite-editor ./path/to/your.db
```

- CLI help:
```bash
sqlite-editor -h
# SQLite3 TUI Editor
# Usage: sqlite-editor [OPTIONS] <DB_PATH>
# Options:
#   -n, --page-size <NUM>  Rows per page (default: 200)
```

## Features
- Fast, smooth browsing
  - Large tables feel responsive with in‑window smooth scrolling
  - Left pane: tables; Right pane: rows of the selected table
- Inline editing
  - Live, inline cell edits with a visible cursor while typing
  - Supports setting NULL quickly; undo the last change
- Powerful filter & sort
  - Case‑insensitive substring filter across all columns
  - Cycle sort on the selected column; toggle ascending/descending
- Copy & export that just works
  - Copy cell, row, or the current page (TSV) to clipboard, with file fallback
  - Export CSV for the current table (respects filter/sort)
- Flexible layout
  - Adjustable column widths; autosize one or all columns
  - Optional cell viewer pane for full wrapped content
- Clear UX
  - Concise keybinds overlay
  - Focus switching between panes
  - Redraws only on state change or tick for a snappy feel

## Usage
1. Start the app with `sqlite-editor /path/to/db.sqlite`.
2. Use the Tables pane (left) to pick a table (↑/↓, Enter).
3. Navigate rows/columns in the Data pane (right).
4. Press `e` to edit a cell, `Enter` to save, or `Esc` to cancel.
5. Press `?` anytime to see keybinds. Press `/` to filter. Use `s`/`S` to sort.
6. Copy page/row/cell or export CSV as needed.

### Handy shortcuts
- Global
  - `q` Quit
  - `r` Reload current table
  - `?` Toggle keybinds
  - `Tab` Switch focus (Tables ⇄ Data)
- Tables
  - `Up/Down` Move selection
  - `Enter` Open selected table
- Data navigation
  - `Left/Right` Move column
  - `Up/Down` or `j/k` Move row
  - `PageUp/PageDown` Previous/Next page
- Editing
  - `e` Edit cell
  - `Enter` Save
  - `Esc` Cancel
  - `Ctrl+d` Set NULL
  - `u` Undo last change (per table, last change in this session)
- Filter
  - `/` Begin filter input
  - `Enter` Apply filter
  - `Esc` Clear filter (also works in normal mode)
- Sorting
  - `s` Cycle sort column (based on current selection)
  - `S` Toggle sort direction (Asc/Desc)
- Copy & export
  - `c` Copy current cell (TSV)
  - `C` Copy current row (TSV)
  - `Ctrl+C` Copy current page (TSV)
  - `E` Export CSV (respects filter/sort)
- Width & viewer
  - `+` or `=` Wider column
  - `-` or `_` Narrower column
  - `a` Autosize current column
  - `A` Autosize all columns
  - `v` Toggle cell viewer pane

## Troubleshooting
- Edits don’t save
  - Editing requires a `rowid`-backed table. Editing the `__rowid__` column itself is not supported.
  - Tables created WITHOUT ROWID are not yet supported for inline edits/undo.
- Clipboard copy doesn’t work
  - The app tries several clipboard tools. Install one:
    - Wayland: `wl-clipboard` (wl-copy)
    - X11: `xclip` or `xsel`
    - macOS: `pbcopy`
    - Windows: built-in `clip`
  - If none is found, the app writes content to a temp file and shows its path.
- Paths starting with a dash
  - Use `--` before the DB path, e.g. `sqlite-editor -- --/path/starting/with/dash.db`.
- UI feels busy or cramped
  - Close overlays (`?`) or the cell viewer (`v`), or reduce visible rows with `-n`.

## Roadmap
- PK/unique‑key aware edits (support for WITHOUT ROWID/composite keys)
- Basic schema viewer and safe schema operations
- Query console tab and richer export formats (TSV/JSON scopes)
- Additional clipboard and export integrations

## Credits
- Built with:
  - [ratatui](https://github.com/ratatui-org/ratatui)
  - [crossterm](https://github.com/crossterm-rs/crossterm)
  - [rusqlite](https://github.com/rusqlite/rusqlite)
- Clipboard helpers: tries common system tools (wl-copy, xclip, xsel, pbcopy, clip).
- Thanks to the Rust and TUI communities for inspiration and examples.

## License
MIT — see [LICENSE](LICENSE).