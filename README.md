# sqlite-editor
A fast, minimal terminal UI (TUI) viewer/editor for SQLite databases written in Rust. It uses ratatui for rendering, crossterm for input, and rusqlite for database access.

Overview
- Left pane: list of tables
- Right pane: paginated rows of the selected table
- Inline cell editing (rowid-backed tables)
- Asynchronous DB worker keeps the UI responsive

Install
Prerequisites:
- Rust toolchain (stable)

From source:
- Clone this repository
- Build and run:
  - Debug: cargo run -- <DB_PATH>
  - Release: cargo build --release
  - Install locally: cargo install --path .

Usage
sqlite-editor [OPTIONS] <DB_PATH>
Options:
- -n, --page-size <NUM>  Rows per page (default: 200)
- -h, --help             Show help

Examples:
- cargo run -- /path/to/database.db
- cargo run -- -n 500 /path/to/database.db
- If your path starts with a dash, insert “--” to end option parsing:
  - cargo run -- -- --/weird/path.db

Key bindings
- Global
  - q: Quit
  - r: Reload current table
- Tables (left pane)
  - Up/Down: Move selection
  - Enter: Load selected table (page 1)
- Data (right pane)
  - Left/Right: Move column
  - Up/Down or k/j: Move row
  - PageUp/PageDown: Previous/Next page
  - e: Edit current cell
  - Enter (in edit mode): Save change
  - Esc (in edit mode): Cancel

Features (current)
- Open a SQLite database file
- Browse tables in the schema (sqlite_internal tables hidden)
- Paginated table view using LIMIT/OFFSET
- Inline cell editing using rowid (safe, simple path)
- Basic type handling for edits (int/real/text); blobs shown as hex
- Responsive UI by delegating DB operations to a worker thread

Limitations (for now)
- Edits require rowid-backed tables; without-rowid tables and composite primary-key updates aren’t implemented yet
- COUNT(*) for total rows may be slow on very large tables; it’s best-effort
- No filters, sorting, or search yet
- No schema editor (add/rename/drop columns, indexes)
- Editing __rowid__ is not supported
- Minimal Unicode/grapheme handling in edit input (byte-wise cursor movement)
- No CSV export/copy-to-clipboard yet

Minimal roadmap
- Filtering and sorting: WHERE and ORDER BY per table, quick find
- Query console tab: run arbitrary SQL, view results
- Primary key-aware edits for tables without rowid
- Better type/affinity handling and NULL entry shortcut
- Column sizing, freezing, and hiding; wider text rendering
- Large-table performance improvements: streaming rows, optional COUNT
- Export: copy selection, CSV export
- Schema operations: safe ALTER TABLE flows
- Undo/redo via transactions; optional “save changes” mode

Troubleshooting
- “unexpected argument ‘--/path…’”: remove the leading dashes from the DB path, or use “--” to end option parsing
- Terminal too small: increase window size; the table view needs reasonable width

Contributing
Open issues or PRs for bugs, feature requests, and improvements. Small focused changes are welcome.
