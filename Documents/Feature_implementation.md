
Feature implementation (Rust TUI)

This document describes the features implemented in the Rust-based terminal UI (TUI) SQLite editor (sqlite-editor), which uses ratatui for rendering, crossterm for input, and rusqlite for database access. It supersedes the VSCode/webview feature list and reflects the current TUI codebase.

Core/runtime integration
- CLI TUI application written in Rust (no VSCode/webview).
- Rendering/input: ratatui + crossterm.
- Database: rusqlite.
- Asynchronous DB worker thread:
  - UI sends requests via crossbeam_channel to a background worker.
  - Worker responds with schema/data/ack messages; UI remains responsive.
- PRAGMA tuning at startup:
  - journal_mode = WAL
  - synchronous = NORMAL
- CLI options:
  - DB path (positional)
  - Page size: -n/--page-size (default 200)

Database connectivity and types
- Uses a single rusqlite Connection in the worker.
- Tables are listed from sqlite_master (non-internal tables only).
- Data loads are paginated (LIMIT/OFFSET).
- The first column in the viewer is always the rowid exposed as "__rowid__".
- Type rendering (to strings):
  - NULL -> "NULL"
  - INTEGER -> stringified i64
  - REAL -> stringified f64
  - TEXT -> UTF-8 lossily decoded String
  - BLOB -> hex as 0x…
- Updates use simple type inference:
  - Try integer, then real, else text
  - No explicit NULL literal shortcut yet

Table browsing and pagination
- Left pane: list of tables (highlight moves with Up/Down).
- Right pane: data grid for the selected table.
- Pagination:
  - Enter: load selected table (page 1)
  - PageDown/PageUp: next/previous page
  - Best-effort total row count displayed (COUNT(*) may be expensive on very large tables)
- Selection in data grid:
  - Move column: Left/Right
  - Move row: Up/Down or j/k
- Column widths are evenly distributed per available width (no resizing yet).

Editing (rowid-based)
- Inline cell editing for rowid-backed tables.
- Start editing: e (on selected cell)
- Edit buffer (single-line) with simple cursor ops:
  - Arrow Left/Right, Home/End, Backspace/Delete
- Save changes: Enter
  - Executes: UPDATE "table" SET "col" = ? WHERE rowid = ?
  - Editing __rowid__ itself is not supported
- Cancel editing: Esc
- Status line reflects progress/results/errors.
- Limitations:
  - Requires a rowid-backed table (WITHOUT ROWID currently not supported).
  - No composite primary key updates.
  - No per-cell type selector; only numeric parsing followed by text fallback.
  - No NULL entry shortcut yet (empty string is text by default).

Status and messaging
- Bottom status line shows:
  - Current mode: NORMAL/EDIT
  - Contextual messages: loading, page info, update status, errors
  - In edit mode, echoes the current buffer content

Keyboard shortcuts

Normal mode
- q: Quit
- r: Reload current table
- Tables (left pane):
  - Up/Down: Move selection
  - Enter: Load selected table (page 1)
- Data (right pane):
  - Left/Right: Move column
  - Up/Down or j/k: Move row
  - PageUp/PageDown: Previous/Next page
  - e: Edit current cell

Editing mode
- Enter: Save change
- Esc: Cancel edit
- Backspace/Delete: Delete char
- Left/Right: Move cursor
- Home/End: Move to start/end of buffer
- Any printable character: insert into buffer

Error handling and resilience
- Errors from the DB worker are propagated to the status line.
- UI keeps running on errors.
- Worker attempts to open the DB at startup and reports a failure via an error response.

Persistence and settings
- No on-disk persistence of UI state yet.
- Page size is a CLI argument (not persisted).
- UI state is held in memory only.

Not implemented yet (planned)
- Filtering/search:
  - WHERE filtering and quick-find across columns
  - ORDER BY sorting
- Query console:
  - Run arbitrary SQL and view results
- Primary-key aware edits (without-rowid / composite PK support)
- Type/affinity-aware editing and explicit NULL shortcut
- Column sizing, freezing, and hiding; improved rendering for long text
- Large-table performance improvements (streaming rows; optional COUNT)
- Export/copy:
  - Copy selection
  - CSV export
- Schema operations:
  - ALTER TABLE helpers (add/rename/drop columns)
  - Index operations (create/drop)
  - Schema/Index viewer
- Undo/redo via transactions; optional “save changes” mode
- File watching/auto reload

Implementation notes (mapping to source)
- src/main.rs
  - CLI parsing, terminal setup, event loop and key handling
  - AppMode Normal/Editing routing
- src/app.rs
  - App state (tables, columns, rows, selection, edit buffer, paging)
  - High-level actions (load schema/table, navigation, editing flow)
- src/db.rs
  - DB worker (rusqlite): schema listing, data page load, UPDATE logic
  - Basic value parsing for UPDATE
  - Row rendering to string
- src/ui.rs
  - ratatui layout: left table list, right data table, bottom status line
  - Selection highlighting
