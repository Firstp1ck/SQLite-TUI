use std::cmp::min;

use crossbeam_channel::{Receiver, Sender};

use crate::db::{DBRequest, DBResponse, SortDir};

#[derive(Debug, Clone, Copy)]
pub enum AppMode {
    Normal,
    Editing {
        row: usize,
        col: usize,
        cursor: usize, // cursor in edit buffer
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Tables,
    Data,
}

pub struct App {
    pub should_quit: bool,

    // UI state
    pub mode: AppMode,
    pub status: String,

    // Schema
    pub tables: Vec<String>,
    pub selected_table: usize,

    // Focus (which pane is active)
    pub focus: Focus,

    // Table data
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub page_size: usize,
    /// Number of rows currently visible in the viewport; <= page_size
    pub visible_rows_per_page: usize,
    /// Starting row index into the current result (for smooth scrolling)
    pub global_row_offset: usize,
    /// Start index into `buffer_rows` of the visible window
    pub view_start: usize,
    /// Buffer of rows loaded from the database (usually page-sized)
    pub buffer_rows: Vec<Vec<String>>,
    /// Global offset corresponding to the first row in `buffer_rows`
    pub buffer_offset: usize,
    /// The last requested global offset used for the current buffer
    pub last_requested_offset: usize,
    pub page: usize,
    pub total_rows: Option<usize>,

    // Cell selection
    pub sel_row: usize,
    pub sel_col: usize,

    // Editing
    pub edit_buffer: String,
    pub edit_is_null: bool,
    /// Stable rowid of the cell being edited (prevents mismatch on view changes)
    pub edit_rowid: Option<i64>,

    // Column width tiers per visible column (0 = narrow, 1 = normal, 2 = wide)
    pub col_width_tiers: Vec<u8>,
    // Optional absolute widths for columns; 0 = not set (UI may derive)
    pub col_abs_widths: Vec<u16>,

    // Autosize requests (picked up by UI layer)
    pub autosize_col_request: Option<usize>,
    pub autosize_all_request: bool,

    // Cell viewer (show full text of current cell)
    pub show_cell_viewer: bool,

    // Filter/Sort
    pub filter: Option<String>,
    pub filter_input: Option<String>,
    pub sort_by: Option<String>,
    pub sort_dir: Option<SortDir>,
    pub select_last_row_on_load: bool,

    // Help overlay
    pub show_help: bool,

    // Channels
    pub req_tx: Sender<DBRequest>,
    pub resp_rx: Receiver<DBResponse>,
}

impl App {
    pub fn new(page_size: usize, req_tx: Sender<DBRequest>, resp_rx: Receiver<DBResponse>) -> Self {
        Self {
            should_quit: false,
            mode: AppMode::Normal,
            status: "Press q to quit. Enter to open table. e to edit cell. PgUp/PgDn to paginate."
                .into(),
            tables: vec![],
            selected_table: 0,
            focus: Focus::Tables,
            columns: vec![],
            rows: vec![],
            page_size,
            visible_rows_per_page: page_size,
            global_row_offset: 0,
            view_start: 0,
            buffer_rows: Vec::new(),
            buffer_offset: 0,
            last_requested_offset: 0,
            page: 0,
            total_rows: None,
            sel_row: 0,
            sel_col: 0,
            edit_buffer: String::new(),
            edit_is_null: false,
            edit_rowid: None,
            col_width_tiers: Vec::new(),
            col_abs_widths: Vec::new(),
            autosize_col_request: None,
            autosize_all_request: false,
            show_cell_viewer: false,
            filter: None,
            filter_input: None,
            sort_by: None,
            sort_dir: None,
            select_last_row_on_load: false,
            show_help: false,
            req_tx,
            resp_rx,
        }
    }

    pub fn request_schema_refresh(&mut self) {
        let _ = self.req_tx.send(DBRequest::LoadSchema);
        self.status = "Loading schema...".into();
    }

    // Focus helpers
    pub fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            Focus::Tables => Focus::Data,
            Focus::Data => Focus::Tables,
        };
    }

    pub fn handle_db_response(&mut self, resp: DBResponse) {
        match resp {
            DBResponse::Schema { tables } => {
                self.tables = tables;
                if self.selected_table >= self.tables.len() {
                    self.selected_table = 0;
                }
                self.status = format!("Loaded {} tables", self.tables.len());
            }
            DBResponse::TableData {
                table,
                columns,
                rows,
                page,
                total_rows,
            } => {
                // Update schema and page meta
                self.columns = columns;
                self.page = page;
                self.total_rows = total_rows;

                // Fill buffer with newly loaded rows and remember where they start
                self.buffer_rows = rows;
                self.buffer_offset = self.last_requested_offset;

                // Compute visible capacity and view window start
                let cap = self
                    .visible_rows_per_page
                    .min(self.buffer_rows.len())
                    .max(1);
                let mut view_start = self.global_row_offset.saturating_sub(self.buffer_offset);
                let max_start = self.buffer_rows.len().saturating_sub(cap);
                if view_start > max_start {
                    view_start = max_start;
                }
                self.view_start = view_start;

                // Project buffer into visible rows window
                self.rows = self
                    .buffer_rows
                    .iter()
                    .skip(self.view_start)
                    .take(cap)
                    .cloned()
                    .collect();

                // Selection handling
                if self.select_last_row_on_load {
                    self.sel_row = cap.saturating_sub(1);
                    self.select_last_row_on_load = false;
                } else {
                    self.sel_row = self.sel_row.min(cap.saturating_sub(1));
                }
                // Keep selected column within bounds
                self.sel_col = self.sel_col.min(self.columns.len().saturating_sub(1));

                // Reset column width tiers and clear absolute widths for each visible column
                self.col_width_tiers = vec![1; self.columns.len()];
                self.col_abs_widths = vec![0; self.columns.len()];
                self.autosize_col_request = None;
                self.autosize_all_request = false;

                self.status = format!(
                    "Viewing {} — page {} ({} rows/page){}",
                    table,
                    page + 1,
                    self.page_size,
                    total_rows
                        .map(|t| format!(", total ~{}", t))
                        .unwrap_or_default()
                );
            }
            DBResponse::CellUpdated { ok, message } => {
                if ok {
                    // Show clearer status for undo operations and refresh the table
                    let is_undo = matches!(message.as_deref(), Some(m) if m.contains("Undo"));
                    self.status = if is_undo {
                        "Undo: applied".into()
                    } else {
                        message.unwrap_or_else(|| "Cell updated".into())
                    };
                    self.reload_current_table();
                } else {
                    let msg = message.unwrap_or_default();
                    if msg.contains("Undo") {
                        self.status = format!("Undo failed: {}", msg);
                    } else {
                        self.status = format!("Update failed: {}", msg);
                    }
                }
            }
            DBResponse::ExportedCSV { ok, path, message } => {
                if ok {
                    self.status = format!("Exported CSV to {}", path);
                } else {
                    self.status = format!("Export failed: {}", message.unwrap_or_default());
                }
            }
            DBResponse::Error(msg) => {
                self.status = format!("Error: {msg}");
            }
        }
    }

    pub fn current_table_name(&self) -> Option<&str> {
        self.tables.get(self.selected_table).map(|s| s.as_str())
    }

    pub fn load_selected_table_page(&mut self, page: usize) {
        if let Some(table) = self.current_table_name().map(|s| s.to_string()) {
            // Keep existing global_row_offset (smooth scroll base); do not reset on reloads
            self.last_requested_offset = self.global_row_offset;
            let _ = self.req_tx.send(DBRequest::LoadTable {
                table,
                page,
                page_size: self.page_size,
                offset_override: Some(self.global_row_offset),
                filter: self.filter.clone(),
                sort_by: self.sort_by.clone(),
                sort_dir: self.sort_dir,
            });
            self.status = "Loading table...".into();
        }
    }

    pub fn reload_current_table(&mut self) {
        self.load_selected_table_page(self.page);
    }

    pub fn move_table_selection_up(&mut self) {
        if self.tables.is_empty() {
            return;
        }
        if self.selected_table == 0 {
            self.selected_table = self.tables.len() - 1;
        } else {
            self.selected_table -= 1;
        }
    }

    pub fn move_table_selection_down(&mut self) {
        if self.tables.is_empty() {
            return;
        }
        self.selected_table = (self.selected_table + 1) % self.tables.len();
    }

    pub fn next_page(&mut self) {
        // Jump by a full page: advance the smooth-scroll base accordingly
        self.global_row_offset = (self.page + 1).saturating_mul(self.page_size);
        self.load_selected_table_page(self.page + 1);
    }

    pub fn prev_page(&mut self) {
        if self.page > 0 {
            // Jump back by a full page: move the smooth-scroll base accordingly
            self.global_row_offset = (self.page - 1).saturating_mul(self.page_size);
            self.load_selected_table_page(self.page - 1);
        }
    }

    // P0: Filter helpers
    pub fn set_filter_string(&mut self, filter: Option<String>) {
        self.filter = filter;
        // Reset to first page when filter changes
        self.load_selected_table_page(0);
    }

    pub fn clear_filter(&mut self) {
        self.set_filter_string(None);
    }

    // Inline filter input state (for visible entry in UI)
    pub fn begin_filter_input(&mut self) {
        self.filter_input = Some(String::new());
    }

    pub fn update_filter_input_char(&mut self, c: char) {
        if let Some(buf) = self.filter_input.as_mut() {
            buf.push(c);
        }
    }

    pub fn backspace_filter_input(&mut self) {
        if let Some(buf) = self.filter_input.as_mut() {
            buf.pop();
        }
    }

    pub fn apply_filter_input(&mut self) {
        let pending = self.filter_input.take();
        match pending {
            Some(s) if !s.is_empty() => self.set_filter_string(Some(s)),
            _ => self.clear_filter(),
        }
    }

    pub fn cancel_filter_input(&mut self) {
        self.filter_input = None;
    }

    // Help overlay toggle
    pub fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
    }

    // P0: Sort helpers
    // Cycle sort for the currently selected column: None -> ASC -> DESC -> None
    pub fn sort_cycle_on_selection(&mut self) {
        if self.columns.is_empty() {
            return;
        }
        let col_name = self.columns[self.sel_col].clone();
        self.sort_by = Some(col_name);
        self.sort_dir = match self.sort_dir {
            None => Some(SortDir::Asc),
            Some(SortDir::Asc) => Some(SortDir::Desc),
            Some(SortDir::Desc) => None,
        };
        self.reload_current_table();
    }

    // Explicitly toggle sort direction (defaults to ASC when not set)
    pub fn sort_toggle_dir(&mut self) {
        self.sort_dir = match self.sort_dir {
            Some(SortDir::Asc) => Some(SortDir::Desc),
            _ => Some(SortDir::Asc),
        };
        self.reload_current_table();
    }

    pub fn move_cell_left(&mut self) {
        if self.columns.is_empty() {
            return;
        }
        self.sel_col = self.sel_col.saturating_sub(1);
    }

    pub fn move_cell_right(&mut self) {
        if self.columns.is_empty() {
            return;
        }
        self.sel_col = min(self.sel_col + 1, self.columns.len().saturating_sub(1));
    }

    pub fn move_cell_up(&mut self) {
        if self.sel_row > 0 {
            self.sel_row = self.sel_row.saturating_sub(1);
            return;
        }
        // At top of visible window: try to scroll within current buffer first
        if self.global_row_offset > self.buffer_offset {
            self.global_row_offset = self.global_row_offset.saturating_sub(1);
            self.view_start = self.view_start.saturating_sub(1);
            let cap = self
                .visible_rows_per_page
                .min(self.buffer_rows.len())
                .max(1);
            if !self.rows.is_empty() && cap == self.rows.len() {
                // shift window up by one: prepend new row and drop last
                let new_row = self.buffer_rows[self.view_start].clone();
                self.rows.pop();
                self.rows.insert(0, new_row);
            } else {
                // fallback rebuild
                self.rows = self
                    .buffer_rows
                    .iter()
                    .skip(self.view_start)
                    .take(cap)
                    .cloned()
                    .collect();
            }
            // Keep cursor at top
            self.sel_row = 0;
            return;
        }
        // Need to load previous buffer
        if self.global_row_offset > 0 {
            self.global_row_offset = self.global_row_offset.saturating_sub(1);
            self.select_last_row_on_load = true;
            self.status = "Loading previous page…".into();
            self.load_selected_table_page(self.global_row_offset / self.page_size);
        }
    }

    pub fn move_cell_down(&mut self) {
        let last_visible = self
            .visible_rows_per_page
            .min(self.rows.len())
            .saturating_sub(1);
        if self.sel_row < last_visible {
            self.sel_row = min(self.sel_row + 1, last_visible);
            return;
        }
        // At bottom of visible window: try to scroll within current buffer first
        let buffer_end = self.buffer_offset.saturating_add(self.buffer_rows.len());
        if self
            .global_row_offset
            .saturating_add(self.sel_row)
            .saturating_add(1)
            < buffer_end
        {
            self.global_row_offset = self.global_row_offset.saturating_add(1);
            self.view_start = self.view_start.saturating_add(1);
            let cap = self
                .visible_rows_per_page
                .min(self.buffer_rows.len())
                .max(1);
            if !self.rows.is_empty() && cap == self.rows.len() {
                // shift window down by one: drop first and append new row
                let new_row = self.buffer_rows[self.view_start + cap - 1].clone();
                self.rows.remove(0);
                self.rows.push(new_row);
            } else {
                // fallback rebuild
                self.rows = self
                    .buffer_rows
                    .iter()
                    .skip(self.view_start)
                    .take(cap)
                    .cloned()
                    .collect();
            }
            // Keep cursor pinned at bottom row
            self.sel_row = last_visible;
            return;
        }
        // Need to load next buffer
        self.global_row_offset = self.global_row_offset.saturating_add(1);
        self.status = "Loading next page…".into();
        self.load_selected_table_page(self.global_row_offset / self.page_size);
    }

    pub fn begin_edit_cell(&mut self) {
        if self.rows.is_empty() || self.columns.is_empty() {
            return;
        }
        let row = self.sel_row;
        let col = self.sel_col;
        // Prevent editing the __rowid__ column and provide a clear status message.
        if self.columns.get(col).map(|s| s.as_str()) == Some("__rowid__") {
            self.status = "Editing __rowid__ is not supported".into();
            return;
        }
        if let AppMode::Editing {
            row: erow,
            col: ecol,
            ..
        } = self.mode
            && erow == row
            && ecol == col
        {
            // Already editing this cell; do not reset buffer or cursor
            self.status = "Editing: Enter to save, Esc to cancel".into();
            return;
        }
        // Capture a stable rowid for this edit session
        let rowid = self
            .rows
            .get(row)
            .and_then(|r| r.first())
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(-1);
        if rowid < 0 {
            self.status = "Invalid rowid; cannot edit this row".into();
            return;
        }
        self.edit_rowid = Some(rowid);

        let current = self
            .rows
            .get(row)
            .and_then(|r| r.get(col))
            .cloned()
            .unwrap_or_default();
        self.edit_buffer = current;
        self.edit_is_null = false;
        self.mode = AppMode::Editing {
            row,
            col,
            cursor: self.edit_buffer.len(),
        };
        self.status = "Editing: Enter to save, Esc to cancel".into();
    }

    pub fn cancel_edit_cell(&mut self) {
        self.mode = AppMode::Normal;
        self.edit_rowid = None;
        self.status = "Edit cancelled".into();
    }

    // P0: Mark current edit to set NULL on submit
    pub fn edit_mark_null(&mut self) {
        if let AppMode::Editing { .. } = self.mode {
            self.edit_is_null = true;
            self.status = "Will set NULL (Enter to save, Esc to cancel)".into();
        }
    }

    pub fn submit_cell_edit(&mut self) {
        let (row, col) = match self.mode {
            AppMode::Normal => return,
            AppMode::Editing { row, col, .. } => (row, col),
        };
        self.mode = AppMode::Normal;

        let Some(table) = self.current_table_name().map(|s| s.to_string()) else {
            return;
        };
        if self.rows.is_empty() || self.columns.is_empty() {
            return;
        }
        // Expect first column to be "__rowid__"
        if self.columns.first().map(|c| c.as_str()) != Some("__rowid__") {
            self.status = "Editing currently requires rowid-backed tables".into();
            return;
        }
        // Use the stable rowid captured when editing began
        let rowid: i64 = self
            .edit_rowid
            .or_else(|| {
                self.rows
                    .get(row)
                    .and_then(|r| r.first())
                    .and_then(|s| s.parse::<i64>().ok())
            })
            .unwrap_or(-1);
        if rowid < 0 {
            self.status = "Invalid rowid; cannot update".into();
            return;
        }

        let col_name = &self.columns[col];
        if col_name == "__rowid__" {
            self.status = "Editing __rowid__ is not supported".into();
            return;
        }

        let new_val = if self.edit_is_null {
            None
        } else {
            Some(self.edit_buffer.clone())
        };
        let _ = self.req_tx.send(DBRequest::UpdateCell {
            table,
            rowid,
            column: col_name.clone(),
            new_value: new_val,
        });
        // Clear the captured rowid after dispatch
        self.edit_rowid = None;
        self.status = "Updating cell...".into();
    }

    // Editing buffer ops
    pub fn edit_input_insert(&mut self, ch: char) {
        if let AppMode::Editing { ref mut cursor, .. } = self.mode {
            self.edit_buffer.insert(*cursor, ch);
            *cursor += ch.len_utf8();
        }
    }
    pub fn edit_input_backspace(&mut self) {
        if let AppMode::Editing { ref mut cursor, .. } = self.mode
            && *cursor > 0
        {
            let new_cursor = prev_grapheme(self.edit_buffer.as_str(), *cursor);
            self.edit_buffer.drain(new_cursor..*cursor);
            *cursor = new_cursor;
        }
    }
    pub fn edit_input_delete(&mut self) {
        if let AppMode::Editing { ref mut cursor, .. } = self.mode
            && *cursor < self.edit_buffer.len()
        {
            let next = next_grapheme(self.edit_buffer.as_str(), *cursor);
            self.edit_buffer.drain(*cursor..next);
        }
    }
    pub fn edit_input_left(&mut self) {
        if let AppMode::Editing { ref mut cursor, .. } = self.mode {
            *cursor = prev_grapheme(self.edit_buffer.as_str(), *cursor);
        }
    }
    pub fn edit_input_right(&mut self) {
        if let AppMode::Editing { ref mut cursor, .. } = self.mode {
            *cursor = next_grapheme(self.edit_buffer.as_str(), *cursor);
        }
    }
    pub fn edit_input_home(&mut self) {
        if let AppMode::Editing { ref mut cursor, .. } = self.mode {
            *cursor = 0;
        }
    }
    pub fn edit_input_end(&mut self) {
        if let AppMode::Editing { ref mut cursor, .. } = self.mode {
            *cursor = self.edit_buffer.len();
        }
    }

    // ===== Copy to clipboard/file helpers (TSV) =====

    /// Copy the currently selected cell as text to clipboard; fallback to a temp .tsv file.
    pub fn copy_current_cell_tsv(&mut self) {
        if self.rows.is_empty() || self.columns.is_empty() {
            self.status = "Nothing to copy (no data)".into();
            return;
        }
        let r = self.sel_row.min(self.rows.len().saturating_sub(1));
        let c = self.sel_col.min(self.columns.len().saturating_sub(1));
        let cell = self
            .rows
            .get(r)
            .and_then(|row| row.get(c))
            .cloned()
            .unwrap_or_default();
        self.copy_to_clipboard_or_file(cell, "cell");
    }

    /// Copy the currently selected row as TSV to clipboard; fallback to a temp .tsv file.
    pub fn copy_current_row_tsv(&mut self) {
        if self.rows.is_empty() || self.columns.is_empty() {
            self.status = "Nothing to copy (no data)".into();
            return;
        }
        let r = self.sel_row.min(self.rows.len().saturating_sub(1));
        let line = self
            .rows
            .get(r)
            .map(|row| row.join("\t"))
            .unwrap_or_default();
        self.copy_to_clipboard_or_file(line, "row");
    }

    /// Copy the current page (with header) as TSV to clipboard; fallback to a temp .tsv file.
    pub fn copy_current_page_tsv(&mut self) {
        if self.rows.is_empty() || self.columns.is_empty() {
            self.status = "Nothing to copy (no data)".into();
            return;
        }
        let mut out = String::new();
        // header
        out.push_str(&self.columns.join("\t"));
        out.push('\n');
        // rows
        for row in &self.rows {
            out.push_str(&row.join("\t"));
            out.push('\n');
        }
        self.copy_to_clipboard_or_file(out, "page");
    }

    /// Best-effort clipboard copy; falls back to writing a temp .tsv file on failure.
    fn copy_to_clipboard_or_file(&mut self, content: String, label: &str) {
        // Try platform clipboards in order
        let candidates: &[(&str, &[&str])] = &[
            // macOS
            ("pbcopy", &[]),
            // Wayland
            ("wl-copy", &[]),
            // X11
            ("xclip", &["-selection", "clipboard"]),
            ("xsel", &["--clipboard", "--input"]),
            // Windows
            ("clip", &[]),
        ];
        for (prog, args) in candidates {
            if self.try_clipboard_prog(prog, args, &content) {
                self.status = format!("Copied {} to clipboard via {}", label, prog);
                return;
            }
        }
        // Fallback: write to temp file
        let mut file_path = std::env::temp_dir();
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        file_path.push(format!("sqlite-editor-{}.tsv", ts));
        match std::fs::write(&file_path, content.as_bytes()) {
            Ok(_) => {
                self.status = format!(
                    "Wrote {} TSV to {} (no clipboard tool found)",
                    label,
                    file_path.display()
                );
            }
            Err(e) => {
                self.status = format!("Failed to write {} TSV: {}", label, e);
            }
        }
    }

    fn try_clipboard_prog(&self, prog: &str, args: &[&str], content: &str) -> bool {
        match std::process::Command::new(prog)
            .args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            Ok(mut child) => {
                if let Some(mut stdin) = child.stdin.take() {
                    use std::io::Write;
                    let _ = stdin.write_all(content.as_bytes());
                }
                if let Ok(status) = child.wait() {
                    return status.success();
                }
                false
            }
            Err(_) => false,
        }
    }

    // ===== Column width tiers (0 = narrow, 1 = normal, 2 = wide) =====

    /// Make the current column narrower by one tier.
    pub fn resize_current_column_narrower(&mut self) {
        if self.columns.is_empty() {
            return;
        }
        let col = self.sel_col.min(self.columns.len().saturating_sub(1));
        if self.col_width_tiers.len() != self.columns.len() {
            self.col_width_tiers = vec![1; self.columns.len()];
        }
        let cur = self.col_width_tiers[col];
        self.col_width_tiers[col] = cur.saturating_sub(1);
    }

    /// Make the current column wider by one tier.
    pub fn resize_current_column_wider(&mut self) {
        if self.columns.is_empty() {
            return;
        }
        let col = self.sel_col.min(self.columns.len().saturating_sub(1));
        if self.col_width_tiers.len() != self.columns.len() {
            self.col_width_tiers = vec![1; self.columns.len()];
        }
        let cur = self.col_width_tiers[col];
        self.col_width_tiers[col] = (cur + 1).min(2);
    }

    /// Expose width tiers (read-only) for rendering logic.
    pub fn column_width_tiers(&self) -> &[u8] {
        &self.col_width_tiers
    }

    // Request autosize for the currently selected column.
    // UI should fulfill this by measuring content and then clearing the request.
    pub fn request_autosize_current_column(&mut self) {
        if self.columns.is_empty() {
            return;
        }
        let col = self.sel_col.min(self.columns.len().saturating_sub(1));
        self.autosize_col_request = Some(col);
    }

    // Request autosize for all visible columns.
    pub fn request_autosize_all_columns(&mut self) {
        self.autosize_all_request = true;
        self.autosize_col_request = None;
    }

    // Toggle a full cell viewer pane to show the complete text of the current cell.
    pub fn toggle_cell_viewer(&mut self) {
        self.show_cell_viewer = !self.show_cell_viewer;
    }

    // Get the current cell's text (for viewer panes).
    pub fn current_cell_text(&self) -> Option<&str> {
        if self.rows.is_empty() || self.columns.is_empty() {
            return None;
        }
        let r = self.sel_row.min(self.rows.len().saturating_sub(1));
        let c = self.sel_col.min(self.columns.len().saturating_sub(1));
        self.rows
            .get(r)
            .and_then(|row| row.get(c))
            .map(|s| s.as_str())
    }
}

// Simplified grapheme stepping without unicode-segmentation:
// moves by bytes; acceptable for a PoC.
fn prev_grapheme(_s: &str, idx: usize) -> usize {
    idx.saturating_sub(1)
}
fn next_grapheme(s: &str, idx: usize) -> usize {
    min(idx + 1, s.len())
}
