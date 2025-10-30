use anyhow::Result;
use crossbeam_channel::{Receiver, Sender};
use rusqlite::{Connection, Row, types::ValueRef};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write};

#[derive(Debug, Clone, Copy)]
pub enum SortDir {
    Asc,
    Desc,
}

#[derive(Debug)]
pub enum DBRequest {
    LoadSchema,
    LoadTable {
        table: String,
        page: usize,
        page_size: usize,
        /// Optional override for row offset (takes precedence over page*page_size)
        offset_override: Option<usize>,
        /// Optional case-insensitive substring filter across all columns
        filter: Option<String>,
        /// Optional sort column (column name or "__rowid__")
        sort_by: Option<String>,
        /// Optional sort direction (defaults to Asc when Some(sort_by) and None here)
        sort_dir: Option<SortDir>,
    },
    UpdateCell {
        table: String,
        rowid: i64,
        column: String,
        /// None means set SQL NULL
        new_value: Option<String>,
    },
    ExportCSV {
        table: String,
        path: String,
        /// Optional case-insensitive substring filter across all columns
        filter: Option<String>,
        /// Optional sort column (column name or "__rowid__")
        sort_by: Option<String>,
        /// Optional sort direction (defaults to Asc when Some(sort_by) and None here)
        sort_dir: Option<SortDir>,
    },
    /// Undo the last change applied to this table in this process
    UndoLastChange {
        table: String,
    },
}

#[derive(Debug)]
pub enum DBResponse {
    Schema {
        tables: Vec<String>,
    },
    TableData {
        table: String,
        columns: Vec<String>,
        rows: Vec<Vec<String>>,
        page: usize,
        total_rows: Option<usize>,
    },
    CellUpdated {
        ok: bool,
        message: Option<String>,
    },
    ExportedCSV {
        ok: bool,
        path: String,
        message: Option<String>,
    },
    Error(String),
}

pub fn start_db_worker(path: String, req_rx: Receiver<DBRequest>, resp_tx: Sender<DBResponse>) {
    let conn = match Connection::open(path) {
        Ok(c) => c,
        Err(e) => {
            let _ = resp_tx.send(DBResponse::Error(format!("Failed to open DB: {e}")));
            return;
        }
    };

    // safemode: faster reading
    let _ = conn.pragma_update(None, "journal_mode", "WAL");
    let _ = conn.pragma_update(None, "synchronous", "NORMAL");

    // Per-table history of updates for undo functionality
    let mut history: HashMap<String, Vec<Change>> = HashMap::new();

    while let Ok(req) = req_rx.recv() {
        let result = match req {
            DBRequest::LoadSchema => load_schema(&conn).map(|tables| DBResponse::Schema { tables }),
            DBRequest::LoadTable {
                table,
                page,
                page_size,
                offset_override,
                filter,
                sort_by,
                sort_dir,
            } => {
                let params = LoadTableParams {
                    table,
                    page,
                    page_size,
                    offset_override,
                    filter,
                    sort_by,
                    sort_dir,
                };
                load_table(&conn, &params)
            }
            DBRequest::UpdateCell {
                table,
                rowid,
                column,
                new_value,
            } => update_cell(&conn, &mut history, &table, rowid, &column, new_value),
            DBRequest::UndoLastChange { table } => undo_last_change(&conn, &mut history, &table),
            DBRequest::ExportCSV {
                table,
                path,
                filter,
                sort_by,
                sort_dir,
            } => export_csv(&conn, &table, &path, filter, sort_by, sort_dir),
        };

        match result {
            Ok(resp) => {
                let _ = resp_tx.send(resp);
            }
            Err(e) => {
                let _ = resp_tx.send(DBResponse::Error(e.to_string()));
            }
        }
    }
}

fn load_schema(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name"
    )?;
    let names = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(names)
}

struct LoadTableParams {
    table: String,
    page: usize,
    page_size: usize,
    offset_override: Option<usize>,
    filter: Option<String>,
    sort_by: Option<String>,
    sort_dir: Option<SortDir>,
}

fn load_table(conn: &Connection, p: &LoadTableParams) -> Result<DBResponse> {
    // unpack params
    let table = p.table.as_str();
    let page = p.page;
    let page_size = p.page_size;
    let offset_override = p.offset_override;
    let filter = p.filter.clone();
    let sort_by = p.sort_by.clone();
    let sort_dir = p.sort_dir;

    // columns
    let mut col_stmt = conn.prepare(&format!("PRAGMA table_info({})", ident(table)))?;
    let mut columns: Vec<String> = vec!["__rowid__".to_string()];
    let mut cols_only: Vec<String> = Vec::new();
    let mut col_rows = col_stmt.query([])?;
    while let Some(row) = col_rows.next()? {
        let name: String = row.get(1)?;
        columns.push(name.clone());
        cols_only.push(name);
    }

    // Build WHERE for filter: case-insensitive substring across all columns (cast to TEXT)
    let mut where_sql = String::new();
    let mut where_params: Vec<rusqlite::types::Value> = Vec::new();
    if let Some(f) = filter.as_ref() {
        let pat = format!("%{}%", f.to_lowercase());
        if !cols_only.is_empty() {
            let ors = cols_only
                .iter()
                .map(|c| format!("LOWER(CAST({} AS TEXT)) LIKE ?", ident(c)))
                .collect::<Vec<_>>()
                .join(" OR ");
            where_sql.push_str(" WHERE ");
            where_sql.push_str(&ors);
            for _ in &cols_only {
                where_params.push(rusqlite::types::Value::Text(pat.clone()));
            }
        }
    }

    // Build ORDER BY
    let mut order_sql = String::new();
    if let Some(col) = sort_by.as_ref() {
        let valid = col == "__rowid__" || cols_only.iter().any(|c| c == col);
        if valid {
            let dir = match sort_dir.unwrap_or(SortDir::Asc) {
                SortDir::Asc => "ASC",
                SortDir::Desc => "DESC",
            };
            let name = if col == "__rowid__" {
                "__rowid__".to_string()
            } else {
                ident(col)
            };
            order_sql = format!(" ORDER BY {} {}", name, dir);
        }
    }

    // data page
    let offset = offset_override.unwrap_or(page * page_size);
    let sql = format!(
        "SELECT rowid as __rowid__, {} FROM {}{}{} LIMIT ? OFFSET ?",
        cols_only
            .iter()
            .map(|c| ident(c))
            .collect::<Vec<_>>()
            .join(", "),
        ident(table),
        where_sql,
        order_sql
    );
    let mut data_stmt = conn.prepare(&sql)?;

    // Bind params: filter params (if any) + limit + offset
    let mut all_params = where_params.clone();
    all_params.push(rusqlite::types::Value::Integer(page_size as i64));
    all_params.push(rusqlite::types::Value::Integer(offset as i64));
    let params_refs: Vec<&dyn rusqlite::ToSql> = all_params
        .iter()
        .map(|v| v as &dyn rusqlite::ToSql)
        .collect();

    let data_iter = data_stmt.query_map(params_refs.as_slice(), |row| {
        row_to_strings(row, columns.len())
    })?;

    let mut rows: Vec<Vec<String>> = Vec::new();
    for r in data_iter {
        rows.push(r?);
    }

    // total count (optional; can be expensive on very large tables)
    let count_sql = format!("SELECT COUNT(*) FROM {}{}", ident(table), where_sql);
    let total_rows: Option<usize> = if where_sql.is_empty() {
        conn.query_row(&count_sql, [], |row| row.get::<_, i64>(0))
            .ok()
            .map(|n| n as usize)
    } else {
        // Reuse the same filter parameters we used for the data query
        let count_params_refs: Vec<&dyn rusqlite::ToSql> = where_params
            .iter()
            .map(|v| v as &dyn rusqlite::ToSql)
            .collect();
        conn.query_row(&count_sql, count_params_refs.as_slice(), |row| {
            row.get::<_, i64>(0)
        })
        .ok()
        .map(|n| n as usize)
    };

    Ok(DBResponse::TableData {
        table: table.to_string(),
        columns,
        rows,
        page,
        total_rows,
    })
}

fn export_csv(
    conn: &Connection,
    table: &str,
    path: &str,
    filter: Option<String>,
    sort_by: Option<String>,
    sort_dir: Option<SortDir>,
) -> Result<DBResponse> {
    // Build columns
    let mut col_stmt = conn.prepare(&format!("PRAGMA table_info({})", ident(table)))?;
    let mut cols_only: Vec<String> = Vec::new();
    let mut col_rows = col_stmt.query([])?;
    while let Some(row) = col_rows.next()? {
        let name: String = row.get(1)?;
        cols_only.push(name);
    }

    // WHERE
    let mut where_sql = String::new();
    let mut where_params: Vec<rusqlite::types::Value> = Vec::new();
    if let Some(f) = filter.as_ref() {
        let pat = format!("%{}%", f.to_lowercase());
        if !cols_only.is_empty() {
            let ors = cols_only
                .iter()
                .map(|c| format!("LOWER(CAST({} AS TEXT)) LIKE ?", ident(c)))
                .collect::<Vec<_>>()
                .join(" OR ");
            where_sql.push_str(" WHERE ");
            where_sql.push_str(&ors);
            for _ in &cols_only {
                where_params.push(rusqlite::types::Value::Text(pat.clone()));
            }
        }
    }

    // ORDER BY
    let mut order_sql = String::new();
    if let Some(col) = sort_by.as_ref() {
        let valid = col == "__rowid__" || cols_only.iter().any(|c| c == col);
        if valid {
            let dir = match sort_dir.unwrap_or(SortDir::Asc) {
                SortDir::Asc => "ASC",
                SortDir::Desc => "DESC",
            };
            let name = if col == "__rowid__" {
                "__rowid__".to_string()
            } else {
                ident(col)
            };
            order_sql = format!(" ORDER BY {} {}", name, dir);
        }
    }

    // Prepare query
    let sql = format!(
        "SELECT rowid as __rowid__, {} FROM {}{}{}",
        cols_only
            .iter()
            .map(|c| ident(c))
            .collect::<Vec<_>>()
            .join(", "),
        ident(table),
        where_sql,
        order_sql
    );
    let mut stmt = conn.prepare(&sql)?;
    let params_refs: Vec<&dyn rusqlite::ToSql> = where_params
        .iter()
        .map(|v| v as &dyn rusqlite::ToSql)
        .collect();

    // Open file
    let file = File::create(path)?;
    let mut w = BufWriter::new(file);

    // Write header
    let mut header = Vec::with_capacity(cols_only.len() + 1);
    header.push("__rowid__".to_string());
    header.extend(cols_only.iter().cloned());
    write_csv_row(&mut w, &header)?;

    // Stream rows
    let mut rows = if params_refs.is_empty() {
        stmt.query([])
    } else {
        stmt.query(params_refs.as_slice())
    }?;
    while let Some(row) = rows.next()? {
        let ncols = header.len();
        let mut values = Vec::with_capacity(ncols);
        for i in 0..ncols {
            let v = row.get_ref(i)?;
            values.push(value_to_string(v));
        }
        write_csv_row(&mut w, &values)?;
    }

    w.flush()?;
    Ok(DBResponse::ExportedCSV {
        ok: true,
        path: path.to_string(),
        message: None,
    })
}
fn write_csv_row<W: Write>(w: &mut W, cols: &[String]) -> std::io::Result<()> {
    let mut first = true;
    for col in cols {
        if !first {
            w.write_all(b",")?;
        }
        first = false;
        let needs_quotes =
            col.contains(',') || col.contains('"') || col.contains('\n') || col.contains('\r');
        if needs_quotes {
            let escaped = col.replace('"', "\"\"");
            w.write_all(b"\"")?;
            w.write_all(escaped.as_bytes())?;
            w.write_all(b"\"")?;
        } else {
            w.write_all(col.as_bytes())?;
        }
    }
    w.write_all(b"\n")?;
    Ok(())
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct Change {
    table: String,
    rowid: i64,
    column: String,
    prev_value: Option<String>,
    new_value: Option<String>,
}

fn update_cell(
    conn: &Connection,
    history: &mut HashMap<String, Vec<Change>>,
    table: &str,
    rowid: i64,
    column: &str,
    new_value: Option<String>,
) -> Result<DBResponse> {
    // Fetch previous value for history
    let prev_value: Option<String> = {
        let sql = format!(
            "SELECT {} FROM {} WHERE rowid = ?1",
            ident(column),
            ident(table)
        );
        let mut stmt_prev = conn.prepare(&sql)?;
        stmt_prev
            .query_row([rowid], |row| {
                let v = row.get_ref(0)?;
                Ok(value_to_opt_string(v))
            })
            .ok()
            .flatten()
    };

    // naive type handling: try to bind as integer/real if it parses, else as text; allow NULL
    let mut stmt = conn.prepare(&format!(
        "UPDATE {} SET {} = ?1 WHERE rowid = ?2",
        ident(table),
        ident(column),
    ))?;
    let mut ok = true;
    let mut msg = None;
    let new_value_clone = new_value.clone();
    let value_param = match new_value_clone {
        None => rusqlite::types::Value::Null,
        Some(ref s) => parse_value(s),
    };
    if let Err(e) = stmt.execute((value_param, rowid)) {
        ok = false;
        msg = Some(e.to_string());
    } else {
        // push to per-table history on success
        let entry = Change {
            table: table.to_string(),
            rowid,
            column: column.to_string(),
            prev_value,
            new_value,
        };
        history.entry(table.to_string()).or_default().push(entry);
    }
    Ok(DBResponse::CellUpdated {
        ok,
        message: msg.or_else(|| Some("OK".into())),
    })
}

fn undo_last_change(
    conn: &Connection,
    history: &mut HashMap<String, Vec<Change>>,
    table: &str,
) -> Result<DBResponse> {
    if let Some(stack) = history.get_mut(table)
        && let Some(change) = stack.pop()
    {
        // Apply reverse update: set column back to previous value
        let mut stmt = conn.prepare(&format!(
            "UPDATE {} SET {} = ?1 WHERE rowid = ?2",
            ident(&change.table),
            ident(&change.column),
        ))?;
        let value_param = match change.prev_value {
            None => rusqlite::types::Value::Null,
            Some(ref s) => parse_value(s),
        };
        match stmt.execute((value_param, change.rowid)) {
            Ok(_) => {
                return Ok(DBResponse::CellUpdated {
                    ok: true,
                    message: Some("Undo applied".into()),
                });
            }
            Err(e) => {
                return Ok(DBResponse::CellUpdated {
                    ok: false,
                    message: Some(format!("Undo failed: {}", e)),
                });
            }
        }
    }
    Ok(DBResponse::CellUpdated {
        ok: false,
        message: Some("Nothing to undo".into()),
    })
}

fn value_to_opt_string(v: ValueRef<'_>) -> Option<String> {
    match v {
        ValueRef::Null => None,
        ValueRef::Integer(i) => Some(i.to_string()),
        ValueRef::Real(f) => Some(format!("{}", f)),
        ValueRef::Text(t) => Some(String::from_utf8_lossy(t).to_string()),
        ValueRef::Blob(b) => Some(format!("0x{}", hex::encode(b))),
    }
}

fn parse_value(s: &str) -> rusqlite::types::Value {
    if let Ok(i) = s.parse::<i64>() {
        return rusqlite::types::Value::Integer(i);
    }
    if let Ok(f) = s.parse::<f64>() {
        return rusqlite::types::Value::Real(f);
    }
    // treat empty as NULL? For safety, keep as TEXT. You can add Ctrl-d shortcut for NULL.
    rusqlite::types::Value::Text(s.to_string())
}

fn row_to_strings(row: &Row, ncols: usize) -> rusqlite::Result<Vec<String>> {
    let mut out = Vec::with_capacity(ncols);
    for i in 0..ncols {
        let v = row.get_ref(i)?;
        out.push(value_to_string(v));
    }
    Ok(out)
}

fn value_to_string(v: ValueRef<'_>) -> String {
    match v {
        ValueRef::Null => "NULL".to_string(),
        ValueRef::Integer(i) => i.to_string(),
        ValueRef::Real(f) => {
            let s = format!("{}", f);
            s
        }
        ValueRef::Text(t) => String::from_utf8_lossy(t).to_string(),
        ValueRef::Blob(b) => format!("0x{}", hex::encode(b)),
    }
}

// Quote identifiers with double-quotes, and escape inner quotes
fn ident(name: &str) -> String {
    let escaped = name.replace('"', "\"\"");
    format!("\"{}\"", escaped)
}

// Minimal hex for blob formatting without new dep; inline tiny impl.
// If you prefer, add `hex = "0.4"` to Cargo.toml instead of this.
mod hex {
    pub fn encode(data: &[u8]) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut s = String::with_capacity(data.len() * 2);
        for &b in data {
            s.push(HEX[(b >> 4) as usize] as char);
            s.push(HEX[(b & 0xf) as usize] as char);
        }
        s
    }
}
