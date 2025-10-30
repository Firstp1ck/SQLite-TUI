use anyhow::Result;
use crossbeam_channel::{Receiver, Sender};
use rusqlite::{Connection, Row, types::ValueRef};

#[derive(Debug)]
pub enum DBRequest {
    LoadSchema,
    LoadTable {
        table: String,
        page: usize,
        page_size: usize,
    },
    UpdateCell {
        table: String,
        rowid: i64,
        column: String,
        new_value: String,
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

    while let Ok(req) = req_rx.recv() {
        let result = match req {
            DBRequest::LoadSchema => load_schema(&conn).map(|tables| DBResponse::Schema { tables }),
            DBRequest::LoadTable {
                table,
                page,
                page_size,
            } => load_table(&conn, &table, page, page_size),
            DBRequest::UpdateCell {
                table,
                rowid,
                column,
                new_value,
            } => update_cell(&conn, &table, rowid, &column, &new_value),
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

fn load_table(conn: &Connection, table: &str, page: usize, page_size: usize) -> Result<DBResponse> {
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

    // data page
    let offset = page * page_size;
    let sql = format!(
        "SELECT rowid as __rowid__, {} FROM {} LIMIT ? OFFSET ?",
        cols_only
            .iter()
            .map(|c| ident(c))
            .collect::<Vec<_>>()
            .join(", "),
        ident(table)
    );
    let mut data_stmt = conn.prepare(&sql)?;
    let data_iter = data_stmt.query_map([page_size as i64, offset as i64], |row| {
        row_to_strings(row, columns.len())
    })?;

    let mut rows: Vec<Vec<String>> = Vec::new();
    for r in data_iter {
        rows.push(r?);
    }

    // total count (optional; can be expensive on huge tables)
    let total_rows = conn
        .query_row(
            &format!("SELECT COUNT(*) FROM {}", ident(table)),
            [],
            |row| row.get::<_, i64>(0),
        )
        .ok()
        .map(|n| n as usize);

    Ok(DBResponse::TableData {
        table: table.to_string(),
        columns,
        rows,
        page,
        total_rows,
    })
}

fn update_cell(
    conn: &Connection,
    table: &str,
    rowid: i64,
    column: &str,
    new_value: &str,
) -> Result<DBResponse> {
    // naive type handling: try to bind as integer/real if it parses, else as text
    // you can improve with pragma_table_info + affinity
    let mut stmt = conn.prepare(&format!(
        "UPDATE {} SET {} = ?1 WHERE rowid = ?2",
        ident(table),
        ident(column),
    ))?;
    let mut ok = true;
    let mut msg = None;
    let value_param = parse_value(new_value);
    if let Err(e) = stmt.execute((value_param, rowid)) {
        ok = false;
        msg = Some(e.to_string());
    }
    Ok(DBResponse::CellUpdated {
        ok,
        message: msg.or_else(|| Some("OK".into())),
    })
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
