
Core integration
- Custom editor for SQLite files: opens .db, .sqlite, .sqlite3 with a single click in VSCode.
- Webview UI retained when hidden; supports multiple editors per document.
- Works in untrusted workspaces; virtual workspaces disabled.
- Git diff: unsupported in-webview; shows a textconv command hint for proper diffs.

Database connectivity/runtime
- Uses a bundled Python server (`vscode/server.py`) launched by the extension.
- Auto-detects Python (configurable with `sqlite3-editor.pythonPath`), requiring Python >= 3.6 and SQLite >= 3.8.
- Falls back for SQLite < 3.37.0 (no `PRAGMA table_list`): still lists tables and disables STRICT appropriately.
- Separate read-only/read-write connections with safe integer handling and a custom `find_widget_regexp` SQL function.

Spreadsheet-like table viewer
- Virtualized reading of table data with LIMIT/OFFSET; custom vertical scrollbar.
- Adjustable visible row count by dragging a bottom handle (min 1, max 200).
- Column header actions:
  - Resize column widths by dragging; persisted per-table.
  - Context menu: Rename…, Delete…, Add Column…, Hide, Copy Column Name.
  - Header shows: type, NOT NULL, DEFAULT, PRIMARY KEY/AUTOINCREMENT, and inline REFERENCES info for foreign keys.
- Cell/row context menus:
  - Cell: Update, Copy Text, Copy JSON, Copy SQL literal.
  - Row gutter: Delete…, Copy Row Number, Copy Rowid.
- Cell rendering:
  - Type coloring for numbers/strings/nulls.
  - BLOBs: hex preview; auto-preview image/audio/video blobs inline when file type is recognized.
  - Preserves 64-bit integers as BigInt when appropriate.
- Visible columns editor:
  - Dialog to toggle which columns are shown in SELECT; enforces at least one column.

Find/search (filtering)
- “Find” widget that filters all rows across all columns via SQL WHERE:
  - Options: case-sensitive, whole-word, regex (implemented with `find_widget_regexp`).
  - Keyboard: Ctrl+F toggles find; Alt+C/W/R toggle case/whole/regex; Escape closes.
- Works alongside custom row count and virtualized paging.

Editing (SQL operations with UI)
- UPDATE (in-place cell editing):
  - Inline textarea overlay; type selector: TEXT, NUMERIC, BLOB, NULL, DEFAULT.
  - BLOB import/export per cell from/to files.
  - WHERE clause selector picks from rowid and unique constraints (PK/UNIQUE) detected from indexes.
  - Keyboard navigation: arrows, PageUp/Down, Home/End, Ctrl+arrows; Tab/Shift+Tab; Enter variants.
  - Commit with Ctrl+Enter; confirmation dialog on unsaved changes; Escape to cancel as context allows.
- INSERT:
  - Editor per column with type selector and default handling; BLOB import/export support.
  - Inserts DEFAULT VALUES when all columns use DEFAULT.
  - Scrolls to bottom after insert.
- DELETE:
  - Row delete (gutter click or context menu) with WHERE built from chosen unique selector.
- ALTER TABLE:
  - RENAME TO, RENAME COLUMN, ADD COLUMN (full column builder), DROP COLUMN.
  - Column builder supports: affinity (TEXT/NUMERIC/INTEGER/REAL/BLOB/ANY with STRICT awareness), PRIMARY KEY, AUTOINCREMENT, UNIQUE, NOT NULL, DEFAULT (expression).
- CREATE TABLE:
  - Multi-column builder; table constraints textarea (e.g., FOREIGN KEY …); STRICT and WITHOUT ROWID toggles.
- DROP TABLE / DROP VIEW.
- CREATE INDEX:
  - UNIQUE option, column list, optional WHERE (partial indexes).
- DROP INDEX.

Schema and indexes view
- Syntax-highlighted table schema (Prism).
- Index list with details:
  - Shows `CREATE INDEX` SQL if available; otherwise prints columns/rowid/expression.
  - Drop index action.

Custom viewer query
- Toggle a “Custom” mode to supply any SELECT subquery for the viewer header.
- Still supports column/record rendering and find, but schema/index/edit operations are appropriately limited when not bound to a single base table.

Auto reload
- Watches the database file and its -wal; notifies/reloads on external changes.
- “Auto reload” toggle.
- Periodic check that reloads all tables when changes are detected.

Other tools (integrated terminal helpers)
- Opens an integrated “SQLite3 Editor” terminal and types prepared commands; auto-installs `sqlite-utils` via pip if missing:
  - Check journal mode (view).
  - Enable WAL mode.
  - Import table from JSON or CSV via sqlite-utils; import from SQL via `sqlite3` CLI.
  - Export table to JSON/CSV via sqlite-utils; export DB to SQL via `sqlite3 .dump`.
  - Copy table via sqlite-utils.
- Placeholders `{{pythonPath}}` and `{{databasePath}}` are expanded automatically.

Persistence and settings
- Remembers last selected table, visible row count, UI toggles/state using VSCode `workspaceState`.
- VSCode setting: `sqlite3-editor.pythonPath` to force a specific Python interpreter.

Error handling and UX niceties
- Error panel with details (query/params on SQL errors).
- Inline invalid query message for viewer when a SELECT is invalid.
- Unsaved changes confirmation dialog with Commit/Discard/Cancel.
- In-webview undo/redo fix for VSCode: Ctrl+Z / Ctrl+Shift+Z (or Ctrl+Y) on inputs.

Keyboard shortcuts (highlights)
- Ctrl+Enter: commit.
- Ctrl+F: open find widget; Alt+C/W/R toggle find options.
- Rich navigation/selection shortcuts in UPDATE mode (arrows, PageUp/Down, Home/End, Tab, Shift+Tab, Enter variants, Escape).
