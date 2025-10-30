use crate::app::{App, AppMode, Focus};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, List, ListItem, Paragraph, Row, Table, Wrap},
};

pub fn draw(f: &mut Frame, app: &mut App) {
    // Layout: when help is visible, allocate an extra pane above the status line
    let constraints = if app.show_help {
        vec![
            Constraint::Min(1),
            Constraint::Length(15),
            Constraint::Length(1),
        ]
    } else {
        vec![Constraint::Min(1), Constraint::Length(1)]
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(f.size());

    let top = chunks[0];
    let (help_area_opt, status_area) = if app.show_help {
        (Some(chunks[1]), chunks[2])
    } else {
        (None, chunks[1])
    };

    let body_constraints = if app.show_cell_viewer {
        vec![
            Constraint::Length(30),
            Constraint::Min(10),
            Constraint::Length(40),
        ]
    } else {
        vec![Constraint::Length(30), Constraint::Min(10)]
    };
    let body_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(body_constraints)
        .split(top);

    draw_tables(f, body_chunks[0], app);
    draw_data(f, body_chunks[1], &mut *app);
    if app.show_cell_viewer && body_chunks.len() > 2 {
        draw_cell_viewer(f, body_chunks[2], app);
    }
    if let Some(help_area) = help_area_opt {
        draw_help(f, help_area, app);
    }
    draw_status(f, status_area, app);
}

fn draw_help(f: &mut Frame, area: Rect, _app: &App) {
    // Concise, readable keybinds
    let lines = vec![
        Line::from(Span::styled(
            "Keybindings",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("Global:        q Quit  | r Reload table  | ? Toggle keybinds"),
        Line::from("Tables:        Up/Down Move selection    | Enter Open selected table"),
        Line::from(
            "Data:          Left/Right Move column    | Up/Down or j/k Move row   | PageUp/PageDown Prev/Next page   | +/- (=/_) Adjust width",
        ),
        Line::from(
            "Editing:       e Edit cell               | Enter Save   | Esc Cancel  | Ctrl-d Set NULL | u Undo last change",
        ),
        Line::from("Filter:        / Begin filter  | Enter Apply  | Esc Clear (also in normal mode)"),
        Line::from("Sorting:       s Cycle sort by column     | S Toggle direction"),
        Line::from("Copy:          c Copy cell | C Copy row | Ctrl+C Copy page (TSV)"),
        Line::from("Autosize:      a Autosize column | A Autosize all"),
        Line::from("Viewer:        v Toggle cell viewer (shows full content)"),
        Line::from("Export:        E Export CSV (type path, Enter to save, Esc to cancel)"),
    ];
    let p =
        Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title("Keybindings"));
    f.render_widget(p, area);
}

fn draw_tables(f: &mut Frame, area: Rect, app: &App) {
    let items: Vec<ListItem> = app
        .tables
        .iter()
        .map(|t| ListItem::new(t.clone()))
        .collect();

    // Visually indicate focus on the Tables pane by changing border color and title
    let title = if app.focus == Focus::Tables { "Tables ◀" } else { "Tables" };
    let block = if app.focus == Focus::Tables {
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title(title)
    } else {
        Block::default()
            .borders(Borders::ALL)
            .title(title)
    };

    let list = List::new(items)
        .block(block)
        .highlight_style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::Yellow),
        );

    f.render_stateful_widget(list, area, &mut list_state(app));
}

fn list_state(app: &App) -> ratatui::widgets::ListState {
    let mut st = ratatui::widgets::ListState::default();
    if !app.tables.is_empty() {
        st.select(Some(app.selected_table));
    }
    st
}

fn draw_status(f: &mut Frame, area: Rect, app: &App) {
    let mode = match app.mode {
        AppMode::Normal => "NORMAL",
        AppMode::Editing { .. } => "EDIT",
    };

    let filter_str = app
        .filter
        .as_ref()
        .map(|s| format!(" | filter: {}", s))
        .unwrap_or_default();

    let sort_str = match (&app.sort_by, app.sort_dir) {
        (Some(col), Some(crate::db::SortDir::Asc)) => format!(" | sort: {} ↑", col),
        (Some(col), Some(crate::db::SortDir::Desc)) => format!(" | sort: {} ↓", col),
        (Some(col), None) => format!(" | sort: {}", col),
        _ => String::new(),
    };

    let text = Line::from(vec![
        Span::styled(
            format!("[{mode}] "),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(&app.status),
        Span::raw(filter_str),
        Span::raw(sort_str),
        match app.mode {
            AppMode::Editing { .. } => Span::raw(format!(" | {}", app.edit_buffer)),
            _ => Span::raw("".to_string()),
        },
    ]);
    let p = Paragraph::new(text).block(Block::default().borders(Borders::TOP));
    f.render_widget(p, area);
}

fn draw_data(f: &mut Frame, area: Rect, app: &mut App) {
    let base_title = if let Some(t) = app.current_table_name() {
        format!("Data — {} (page {})", t, app.page + 1)
    } else {
        "Data".to_string()
    };
    let title = if app.focus == Focus::Data { format!("{base_title} ◀") } else { base_title };
    let block = if app.focus == Focus::Data {
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title(title)
    } else {
        Block::default().borders(Borders::ALL).title(title)
    };
    if app.columns.is_empty() {
        let p = Paragraph::new("Select a table and press Enter").block(block);
        f.render_widget(p, area);
        return;
    }

    // Compute inner area then render outer block
    let inner = block.inner(area);
    f.render_widget(block, area);
    let inner_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)].as_ref())
        .split(inner);

    // Filter bar
    let filter_text = if let Some(buf) = app.filter_input.as_ref() {
        format!("Filter: {}_   (Enter to apply, Esc to clear)", buf)
    } else if let Some(s) = app.filter.as_ref() {
        format!("Filter: {}   (Esc to clear)", s)
    } else {
        "Filter: (none)   (/ to filter)".to_string()
    };
    let filter_line = Paragraph::new(filter_text).style(Style::default().fg(Color::DarkGray));
    f.render_widget(filter_line, inner_chunks[0]);

    // Update visible rows per page (capacity = table area height - header)
    let table_area_height = inner_chunks[1].height;
    let header_lines: u16 = 1;
    let capacity = table_area_height.saturating_sub(header_lines) as usize;
    app.visible_rows_per_page = capacity.max(1).min(app.page_size);

    // Fulfill autosize requests (if any)
    {
        let cols = app.columns.len();
        if app.col_abs_widths.len() != cols {
            app.col_abs_widths = vec![0; cols];
        }
        if app.autosize_all_request {
            for i in 0..cols {
                app.col_abs_widths[i] = measure_column_width(app, i);
            }
            app.autosize_all_request = false;
            app.autosize_col_request = None;
        } else if let Some(i) = app.autosize_col_request.take()
            && i < cols {
                app.col_abs_widths[i] = measure_column_width(app, i);
            }
    }
    // Table inside inner area
    let widths = column_widths(
        inner.width,
        app.columns.len(),
        app.column_width_tiers(),
        &app.col_abs_widths,
    );
    let header = Row::new(app.columns.iter().map(|c| Cell::from(c.as_str()))).style(
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    );

    let mut rows = Vec::with_capacity(app.rows.len());
    for (r_idx, row) in app.rows.iter().enumerate() {
        let mut cells = Vec::with_capacity(row.len());
        for (c_idx, val) in row.iter().enumerate() {
            // Live editing view: render edit buffer with a visible cursor for the editing cell.
            let mut cell = if let AppMode::Editing { row: erow, col: ecol, cursor } = app.mode {
                if r_idx == erow && c_idx == ecol {
                    let buf = app.edit_buffer.as_str();
                    let cur = cursor.min(buf.len());
                    let (left, right) = buf.split_at(cur);
                    let line = Line::from(vec![Span::raw(left), Span::raw("▏"), Span::raw(right)]);
                    Cell::from(line)
                } else {
                    Cell::from(val.as_str())
                }
            } else {
                Cell::from(val.as_str())
            };

            // Highlight selection, and use a distinct highlight for the editing cell.
            if let AppMode::Editing { row: erow, col: ecol, .. } = app.mode {
                if r_idx == erow && c_idx == ecol {
                    cell = cell.style(Style::default().bg(Color::Yellow).fg(Color::Black));
                } else if r_idx == app.sel_row && c_idx == app.sel_col {
                    cell = cell.style(Style::default().bg(Color::Blue).fg(Color::Black));
                }
            } else if r_idx == app.sel_row && c_idx == app.sel_col {
                cell = cell.style(Style::default().bg(Color::Blue).fg(Color::Black));
            }

            cells.push(cell);
        }
        rows.push(Row::new(cells));
    }

    let table = Table::new(rows, widths).header(header).column_spacing(1);

    f.render_widget(table, inner_chunks[1]);
}

fn column_widths(total_width: u16, cols: usize, tiers: &[u8], abs: &[u16]) -> Vec<Constraint> {
    if cols == 0 {
        return vec![];
    }
    // Use absolute widths when provided (>0), otherwise fall back to tier-based ratios.
    let mut any_abs = false;
    let mut constraints = Vec::with_capacity(cols);
    for i in 0..cols {
        let w = abs.get(i).copied().unwrap_or(0);
        if w > 0 {
            any_abs = true;
        }
        constraints.push(w);
    }
    if any_abs {
        // Clamp individual absolute widths to available area to avoid zero/overflow.
        let min_width: u16 = 3;
        return constraints
            .into_iter()
            .map(|w| {
                if w > 0 {
                    Constraint::Length(w.max(min_width).min(total_width.saturating_sub(1)))
                } else {
                    // No absolute width: assign flexible remainder with a small ratio
                    Constraint::Ratio(1, (cols as u32).max(1))
                }
            })
            .collect();
    }
    // Map tiers to weights: 0=narrow(1), 1=normal(2), 2=wide(3)
    let weights: Vec<u32> = (0..cols)
        .map(|i| match tiers.get(i).copied().unwrap_or(1) {
            0 => 1,
            1 => 2,
            2 => 3,
            _ => 2,
        })
        .collect();
    let sum: u32 = weights.iter().sum();
    if sum == 0 {
        return (0..cols)
            .map(|_| Constraint::Ratio(1, cols as u32))
            .collect();
    }
    weights
        .into_iter()
        .map(|w| Constraint::Ratio(w, sum))
        .collect()
}



// Measure the width (in characters) required to fully display a column,
// considering both header and current page rows. Adds small padding.
fn measure_column_width(app: &App, col: usize) -> u16 {
    if app.columns.is_empty() {
        return 0;
    }
    let mut max_len = app.columns.get(col).map(|s| s.chars().count()).unwrap_or(0);
    for row in &app.rows {
        if let Some(cell) = row.get(col) {
            let l = cell.chars().count();
            if l > max_len {
                max_len = l;
            }
        }
    }
    let padding: usize = 2;
    (max_len.saturating_add(padding)) as u16
}

// Draw a right-side viewer pane that shows the full content of the current cell.
fn draw_cell_viewer(f: &mut Frame, area: Rect, app: &App) {
    let title = "Cell";
    let content = app.current_cell_text().unwrap_or("<empty>");
    let p = Paragraph::new(content)
        .block(Block::default().borders(Borders::ALL).title(title))
        .wrap(Wrap { trim: false })
        .style(Style::default());
    f.render_widget(p, area);
}
