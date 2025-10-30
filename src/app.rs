use std::cmp::min;

use crossbeam_channel::{Receiver, Sender};

use crate::db::{DBRequest, DBResponse};

#[derive(Debug, Clone, Copy)]
pub enum AppMode {
    Normal,
    Editing {
        row: usize,
        col: usize,
        cursor: usize, // cursor in edit buffer
    },
}

pub struct App {
    pub should_quit: bool,

    // UI state
    pub mode: AppMode,
    pub status: String,

    // Schema
    pub tables: Vec<String>,
    pub selected_table: usize,

    // Table data
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub page_size: usize,
    pub page: usize,
    pub total_rows: Option<usize>,

    // Cell selection
    pub sel_row: usize,
    pub sel_col: usize,

    // Editing
    pub edit_buffer: String,

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
            columns: vec![],
            rows: vec![],
            page_size,
            page: 0,
            total_rows: None,
            sel_row: 0,
            sel_col: 0,
            edit_buffer: String::new(),
            req_tx,
            resp_rx,
        }
    }

    pub fn request_schema_refresh(&mut self) {
        let _ = self.req_tx.send(DBRequest::LoadSchema);
        self.status = "Loading schema...".into();
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
                // Keep "__rowid__" hidden in header? We'll show it, but keep selection logic aware
                self.columns = columns;
                self.rows = rows;
                self.page = page;
                self.total_rows = total_rows;
                self.sel_row = 0;
                self.sel_col = 0;
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
                    self.status = message.unwrap_or_else(|| "Cell updated".into());
                    self.reload_current_table();
                } else {
                    self.status = format!("Update failed: {}", message.unwrap_or_default());
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
            let _ = self.req_tx.send(DBRequest::LoadTable {
                table,
                page,
                page_size: self.page_size,
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
        self.load_selected_table_page(self.page + 1);
    }

    pub fn prev_page(&mut self) {
        if self.page > 0 {
            self.load_selected_table_page(self.page - 1);
        }
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
        self.sel_row = self.sel_row.saturating_sub(1);
    }

    pub fn move_cell_down(&mut self) {
        self.sel_row = min(self.sel_row + 1, self.rows.len().saturating_sub(1));
    }

    pub fn begin_edit_cell(&mut self) {
        if self.rows.is_empty() || self.columns.is_empty() {
            return;
        }
        let row = self.sel_row;
        let col = self.sel_col;
        let current = self
            .rows
            .get(row)
            .and_then(|r| r.get(col))
            .cloned()
            .unwrap_or_default();
        self.edit_buffer = current;
        self.mode = AppMode::Editing {
            row,
            col,
            cursor: self.edit_buffer.len(),
        };
        self.status = "Editing: Enter to save, Esc to cancel".into();
    }

    pub fn cancel_edit_cell(&mut self) {
        self.mode = AppMode::Normal;
        self.status = "Edit cancelled".into();
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
        let row_vec = &self.rows[row];
        let rowid: i64 = row_vec
            .first()
            .and_then(|s| s.parse::<i64>().ok())
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

        let new_val = self.edit_buffer.clone();
        let _ = self.req_tx.send(DBRequest::UpdateCell {
            table,
            rowid,
            column: col_name.clone(),
            new_value: new_val,
        });
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
            && *cursor > 0 {
                let new_cursor = prev_grapheme(self.edit_buffer.as_str(), *cursor);
                self.edit_buffer.drain(new_cursor..*cursor);
                *cursor = new_cursor;
            }
    }
    pub fn edit_input_delete(&mut self) {
        if let AppMode::Editing { ref mut cursor, .. } = self.mode
            && *cursor < self.edit_buffer.len() {
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
}

// Simplified grapheme stepping without unicode-segmentation:
// moves by bytes; acceptable for a PoC.
fn prev_grapheme(_s: &str, idx: usize) -> usize {
    idx.saturating_sub(1)
}
fn next_grapheme(s: &str, idx: usize) -> usize {
    min(idx + 1, s.len())
}
