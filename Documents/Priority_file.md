# Priority P0 – Immediate usability wins

- [x] Quick Find / Filter (WHERE)
  - Implemented a lightweight row filter with inline filter bar.
  - Keyboard:
    - / begin filter input; visible as “Filter: …_” beneath Data header
    - Enter apply filter
    - Esc clear filter (works during entry and from normal mode)
  - Behavior:
    - Case-insensitive substring across all columns (casts to TEXT)
    - Pagination resets to first page on change
    - Status shows applied/cleared messages
  - Impl: added filter and filter_input state in App; WHERE composed in db loader; inline filter bar in UI.

- [x] Sort by column (ORDER BY)
  - Keyboard:
    - s cycle sort on the selected column: None → ASC → DESC → None
    - S toggle direction (defaults to ASC if unset)
  - Impl: (sort_by, sort_dir) in App; ORDER BY in db loader; indicators in status bar.

- [x] Copy selection to clipboard/file
  - Keyboard:
    - c copy current cell
    - C copy current row (TSV line)
    - Ctrl+C copy current page (header + rows as TSV)
  - Behavior:
    - Best-effort clipboard (pbcopy, wl-copy, xclip/xsel, clip)
    - Fallback writes TSV to a temp file and shows the path
  - Impl: helpers in App; TSV formatting in App; status feedback.

- [x] CSV export (fast path)
  - Keyboard: E (then type path; Enter to export; Esc cancel)
  - Behavior: Respects current filter/sort; streams rows to disk with header
  - Impl: db worker ExportCSV; UI prompt and status feedback.

- [x] NULL entry shortcut in edit mode
  - Keyboard: Ctrl-d in edit mode sets the field to NULL
  - Impl: Option<String> payload for updates; NULL binding in db worker; status feedback.

- [x] Help overlay and status improvements
  - Keyboard: ? toggle a bottom “Keybindings” pane
  - Status shows: mode, messages, filter/sort indicators, page info
  - Impl: help pane in UI; richer status; Data block title shows table/page.

Extra P0 refinements completed
- Inline filter bar under Data header (always visible):
  - Shows live input cursor during typing
  - Shows current filter and hints: “(Enter to apply, Esc to clear)” or “(/ to filter)”
- Clearing filter with Esc in normal mode (without being in typing mode)
- Table layout renders block, filter bar, and table in the inner area
- Robust key handling fixes


# Priority P1 – Enhanced editing and viewing

- [ ] Primary key–aware updates (non-rowid tables)
  - Use INTEGER PRIMARY KEY alias (rowid) or UNIQUE/PK columns for WHERE
  - Impl notes: PRAGMA table_info + index_list/info; unique selector builder

- [~] Column hiding and width control
  - Width control implemented:
    - Keyboard:
      - + / = widen current column by tier
      - - / _ narrow current column by tier
      - a autosize current column (measures header + page content)
      - A autosize all columns
      - v toggle right-hand cell viewer (shows full, wrapped content)
    - Behavior:
      - Tiers: relative ratios for quick adjustments (0=narrow,1=normal,2=wide)
      - Autosize sets absolute width per column (takes precedence over tiers)
      - Viewer reveals complete text of the current cell without resizing table
    - Remaining:
      - Column hiding (H) and unhide prompt (Shift+H)
  - Impl notes: col_width_tiers + col_abs_widths, autosize requests, viewer pane

- [ ] Persisted session basics
  - Remember last table, page size, sort/filter, and column widths (per table)
  - Impl notes: store in a small config file under $XDG_CONFIG_HOME/sqlite-editor

- [ ] Basic schema viewer (read-only)
  - Keyboard: i show CREATE TABLE and indexes in a side panel
  - Impl notes: sqlite_master; PRAGMA index_list/index_info; UI panel


# Priority P2 – Power-user workflows

- [ ] Query console (read-only first)
  - Keyboard: : open console; Enter run; Esc close
  - Impl notes: AppMode::Console; db runs SELECT; view results in grid

- [ ] Better type/affinity handling in edit
  - Use column affinity to guide INTEGER/REAL/TEXT/NULL; status indicator
  - Keyboard: maybe t to cycle inferred type for edit buffer

- [ ] Large-table performance options
  - Optional COUNT(*) toggle; show “~unknown” when disabled
  - Streaming/chunked exports

- [ ] Export UX
  - Formats: CSV / TSV / JSON; scopes: cell/row/page/table
  - Impl: JSON array of records; TSV = CSV with tabs


# Priority P3 – Advanced features and safety

- [ ] Schema operations (safe)
  - Add/rename/drop column, rename table; preview SQL, warnings, confirm
  - Impl: transactional flows; table re-creation pattern when necessary

- [ ] Index operations
  - Create/drop wizard; refresh schema

- [ ] Undo/redo via transactions
  - u undo, U redo (when enabled)
  - Impl: track savepoints/transactions

- [ ] File watching/auto-reload
  - Detect external changes; offer reload


# Suggested keymap summary (updated)

- Filter: / begin; Enter apply; Esc clear
- Sort: s sort by selected column; S toggle direction
- Copy: c cell; C row; Ctrl+C page (TSV)
- Export: E (prompt for path; CSV; respects filter/sort)
- Help: ? toggle keybindings pane
- Edit: e edit cell; Enter save; Esc cancel; Ctrl-d set NULL
- Width:
  - +/- (=/_) adjust width tier of current column
  - a autosize current column
  - A autosize all columns
- Viewer: v toggle cell viewer
- Navigation:
  - Tables: Up/Down, Enter
  - Data: Left/Right move column; Up/Down or j/k move row; PageUp/PageDown pages
- Global: r reload table; q quit


# Implementation sketch (current vs planned)

- src/app.rs (implemented)
  - State: filter, filter_input, sort_by/sort_dir, width tiers (col_width_tiers), absolute widths (col_abs_widths), autosize requests (autosize_col_request/autosize_all_request), show_cell_viewer
  - Actions: copy helpers (cell/row/page TSV), export dispatch, width adjusters (+/-), autosize requests (a/A), cell viewer toggle (v), filter helpers

- src/db.rs (implemented)
  - Load table: WHERE (case-insensitive LIKE across all columns), ORDER BY; bound params
  - CSV export endpoint; streams rows; includes header; respects filter/sort
  - Update: supports NULL binding via Option<String>

- src/ui.rs (implemented)
  - Data block shows inline filter bar and table in inner area
  - Help pane with grouped keybinds
  - Cell viewer pane (right side) with wrapped content
  - Column widths: absolute width per column when set; falls back to tier-based ratios

- src/main.rs (implemented)
  - Key handling for:
    - Filter: /, Enter, Esc
    - Sort: s/S
    - Copy: c/C/Ctrl+C
    - Export: E (prompt)
    - Width: +/=/−/_; autosize a/A; viewer v
    - Help: ?
    - Edit: e, Enter, Esc, Ctrl-d
    - Navigation: arrows, j/k, PageUp/PageDown; Enter open table
    - Global: r reload; q quit

- Next (planned)
  - Column hiding/unhiding (H / Shift+H) with visible column management UI
  - Persist widths, last table/page size/sort/filter in config file
  - PK/unique-aware updates for tables without rowid
  - Read-only schema viewer
  - Query console, type/affinity feedback, perf toggles, richer export formats