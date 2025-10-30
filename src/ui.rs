use crate::app::{App, AppMode};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, List, ListItem, Paragraph, Row, Table},
};

pub fn draw(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)].as_ref())
        .split(f.size());

    let top = chunks[0];
    let status_area = chunks[1];

    let body_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(30), Constraint::Min(10)].as_ref())
        .split(top);

    draw_tables(f, body_chunks[0], app);
    draw_data(f, body_chunks[1], app);
    draw_status(f, status_area, app);
}

fn draw_tables(f: &mut Frame, area: Rect, app: &App) {
    let items: Vec<ListItem> = app
        .tables
        .iter()
        .map(|t| ListItem::new(t.clone()))
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Tables"))
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
    let text = Line::from(vec![
        Span::styled(
            format!("[{mode}] "),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(&app.status),
        match app.mode {
            AppMode::Editing { .. } => Span::raw(format!(" | {}", app.edit_buffer)),
            _ => Span::raw("".to_string()),
        },
    ]);
    let p = Paragraph::new(text).block(Block::default().borders(Borders::TOP));
    f.render_widget(p, area);
}

fn draw_data(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default().borders(Borders::ALL).title("Data");
    if app.columns.is_empty() {
        let p = Paragraph::new("Select a table and press Enter").block(block);
        f.render_widget(p, area);
        return;
    }

    let widths = column_widths(area.width, app.columns.len());
    let header = Row::new(app.columns.iter().map(|c| Cell::from(c.as_str()))).style(
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    );

    let mut rows = Vec::with_capacity(app.rows.len());
    for (r_idx, row) in app.rows.iter().enumerate() {
        let mut cells = Vec::with_capacity(row.len());
        for (c_idx, val) in row.iter().enumerate() {
            let mut cell = Cell::from(val.as_str());
            if r_idx == app.sel_row && c_idx == app.sel_col {
                cell = cell.style(Style::default().bg(Color::Blue).fg(Color::Black));
            }
            cells.push(cell);
        }
        rows.push(Row::new(cells));
    }

    let table = Table::new(rows, widths)
        .header(header)
        .block(block)
        .column_spacing(1);

    f.render_widget(table, area);
}

fn column_widths(total_width: u16, cols: usize) -> Vec<Constraint> {
    if cols == 0 {
        return vec![];
    }
    // Evenly divide; simple heuristic. You can improve with content-based sizing.
    let w = total_width.saturating_sub(2 + (cols as u16 - 1)); // borders and spacing
    let per = if cols as u16 > 0 {
        max(1, w / cols as u16)
    } else {
        1
    };
    (0..cols).map(|_| Constraint::Length(per)).collect()
}

fn max(a: u16, b: u16) -> u16 {
    if a > b { a } else { b }
}
