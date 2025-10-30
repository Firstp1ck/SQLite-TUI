use std::io;
use std::time::{Duration, Instant};

use anyhow::Result;
use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{backend::CrosstermBackend, prelude::*};

mod app;
mod db;
mod ui;

use app::{App, AppMode};
use db::{DBRequest, DBResponse, start_db_worker};

#[derive(Parser, Debug)]
#[command(author, version, about = "SQLite3 TUI Editor")]
struct Args {
    /// Path to SQLite database file
    #[arg(value_name = "DB_PATH")]
    db_path: String,

    /// Page size (rows per page)
    #[arg(short = 'n', long, default_value_t = 200)]
    page_size: usize,
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

fn restore_terminal(mut terminal: Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}

fn main() -> Result<()> {
    let args = Args::parse();

    let mut terminal = setup_terminal()?;

    // DB worker channels
    let (req_tx, req_rx) = crossbeam_channel::unbounded::<DBRequest>();
    let (resp_tx, resp_rx) = crossbeam_channel::unbounded::<DBResponse>();

    // Start DB worker
    let db_path = args.db_path.clone();
    std::thread::spawn(move || start_db_worker(db_path, req_rx, resp_tx));

    // Initialize app state
    let mut app = App::new(args.page_size, req_tx, resp_rx);
    app.status = "Press ? for help — / filter | s/S sort | +/- (=/_) width | a/A autosize | v view cell | c/C/Ctrl+C copy | E export CSV | e edit | Ctrl-d NULL (edit) | u undo".into();
    app.request_schema_refresh();

    let mut last_tick = Instant::now();
    let tick_rate = Duration::from_millis(100);

    let res = run_app(&mut terminal, &mut app, tick_rate, &mut last_tick);

    restore_terminal(terminal)?;
    if let Err(e) = res {
        eprintln!("Error: {e:?}");
    }
    Ok(())
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    tick_rate: Duration,
    last_tick: &mut Instant,
) -> Result<()> {
    let mut filter_mode = false;
    let mut export_mode = false;
    let mut export_path_buf = String::new();
    // Redraw only when state changes or on tick
    let mut dirty = true;
    loop {
        // Process any DB responses without blocking
        while let Ok(msg) = app.resp_rx.try_recv() {
            match msg {
                DBResponse::ExportedCSV { ok, path, message } => {
                    if ok {
                        app.status = format!("Exported CSV to {}", path);
                    } else {
                        app.status = format!(
                            "Export failed: {}",
                            message.unwrap_or_else(|| "unknown error".into())
                        );
                    }
                    dirty = true;
                }
                _ => {
                    app.handle_db_response(msg);
                    dirty = true;
                }
            }
        }

        let tick_due = last_tick.elapsed() >= tick_rate;
        if dirty || tick_due {
            terminal.draw(|f| ui::draw(f, app))?;
            dirty = false;
            if tick_due {
                *last_tick = Instant::now();
            }
        }

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or(Duration::from_secs(0));

        let should_exit = if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if export_mode {
                    use crossterm::event::{KeyCode::*, KeyModifiers};
                    match key.code {
                        Enter => {
                            if export_path_buf.is_empty() {
                                app.status = "Export cancelled".into();
                            } else if let Some(table) =
                                app.current_table_name().map(|s| s.to_string())
                            {
                                let _ = app.req_tx.send(DBRequest::ExportCSV {
                                    table,
                                    path: export_path_buf.clone(),
                                    filter: app.filter.clone(),
                                    sort_by: app.sort_by.clone(),
                                    sort_dir: app.sort_dir,
                                });
                                app.status = format!("Exporting CSV to {}...", export_path_buf);
                            } else {
                                app.status = "No table selected for export".into();
                            }
                            export_mode = false;
                            export_path_buf.clear();
                        }
                        Esc => {
                            export_mode = false;
                            export_path_buf.clear();
                            app.status = "Export cancelled".into();
                        }
                        Backspace => {
                            export_path_buf.pop();
                            app.status = format!("Export CSV: {}_", export_path_buf);
                        }
                        Char(c) => {
                            if !key.modifiers.contains(KeyModifiers::CONTROL) {
                                export_path_buf.push(c);
                                app.status = format!("Export CSV: {}_", export_path_buf);
                            }
                        }
                        _ => {}
                    }
                    dirty = true;
                    false
                } else if filter_mode {
                    use crossterm::event::{KeyCode::*, KeyModifiers};
                    match key.code {
                        Enter => {
                            // Apply pending input to filter (or clear if empty)
                            app.apply_filter_input();
                            filter_mode = false;
                            app.status = match &app.filter {
                                Some(s) => format!("Filter applied: {}", s),
                                None => "Filter cleared".into(),
                            };
                        }
                        Esc => {
                            // Cancel input and clear active filter
                            app.cancel_filter_input();
                            app.clear_filter();
                            filter_mode = false;
                            app.status = "Filter cleared".into();
                        }
                        Backspace => {
                            app.backspace_filter_input();
                            if let Some(buf) = &app.filter_input {
                                app.status = format!("Filter: {}_", buf);
                            } else {
                                app.status = "Filter: _".into();
                            }
                        }
                        Char(c) => {
                            if !key.modifiers.contains(KeyModifiers::CONTROL) {
                                app.update_filter_input_char(c);
                                if let Some(buf) = &app.filter_input {
                                    app.status = format!("Filter: {}_", buf);
                                }
                            }
                        }
                        _ => {}
                    }
                    dirty = true;
                    false
                } else {
                    match app.mode {
                        AppMode::Normal => match key.code {
                            KeyCode::Char('/') => {
                                filter_mode = true;
                                app.begin_filter_input();
                                app.status =
                                    "Filter: type and Enter to apply (Esc to clear)".into();
                                dirty = true;
                                false
                            }
                            KeyCode::Char('?') => {
                                app.toggle_help();
                                if app.show_help {
                                    app.status = "Showing keybinds (press ? to close)".into();
                                } else {
                                    app.status = "Closed keybinds".into();
                                }
                                dirty = true;
                                false
                            }
                            KeyCode::Char('s') => {
                                app.sort_cycle_on_selection();
                                app.status = "Sort: cycled on selected column".into();
                                dirty = true;
                                false
                            }
                            KeyCode::Char('S') => {
                                app.sort_toggle_dir();
                                app.status = "Sort: direction toggled".into();
                                dirty = true;
                                false
                            }
                            KeyCode::Char('E') => {
                                export_mode = true;
                                export_path_buf.clear();
                                app.status =
                                    "Export CSV path: type and Enter to save (Esc to cancel)"
                                        .into();
                                dirty = true;
                                false
                            }
                            KeyCode::Esc => {
                                if app.filter.is_some() || app.filter_input.is_some() {
                                    app.cancel_filter_input();
                                    app.clear_filter();
                                    app.status = "Filter cleared".into();
                                }
                                dirty = true;
                                false
                            }
                            _ => {
                                if key
                                    .modifiers
                                    .contains(crossterm::event::KeyModifiers::CONTROL)
                                {
                                    if let KeyCode::Char('c') = key.code {
                                        app.copy_current_page_tsv();
                                        dirty = true;
                                        false
                                    } else {
                                        let r = handle_key_normal(app, key.code);
                                        dirty = true;
                                        r
                                    }
                                } else {
                                    let r = handle_key_normal(app, key.code);
                                    dirty = true;
                                    r
                                }
                            }
                        },
                        AppMode::Editing { .. } => {
                            use crossterm::event::KeyCode::*;
                            // Mark dirty only for keys that affect the edit buffer/cursor/state
                            match key.code {
                                Enter | Esc | Backspace | Delete | Left | Right | Home | End | Char(_) => {
                                    dirty = true;
                                }
                                _ => {}
                            }
                            handle_key_editing(app, key)
                        }
                    }
                }
            } else {
                false
            }
        } else {
            false
        };

        if should_exit {
            return Ok(());
        }

        // periodic tasks if needed (last_tick is updated when a tick-driven redraw occurs)

        if app.should_quit {
            return Ok(());
        }
    }
}

fn handle_key_normal(app: &mut App, code: KeyCode) -> bool {
    match code {
        KeyCode::Char('q') => {
            app.should_quit = true;
        }
        KeyCode::Up => {
            if app.focus == app::Focus::Tables {
                app.move_table_selection_up()
            } else {
                app.move_cell_up()
            }
        },
        KeyCode::Down => {
            if app.focus == app::Focus::Tables {
                app.move_table_selection_down()
            } else {
                app.move_cell_down()
            }
        },
        KeyCode::Tab => { app.toggle_focus(); },
        KeyCode::Enter => app.load_selected_table_page(0),
        KeyCode::PageDown => app.next_page(),
        KeyCode::PageUp => app.prev_page(),
        KeyCode::Left => app.move_cell_left(),
        KeyCode::Right => app.move_cell_right(),
        KeyCode::Char('j') => app.move_cell_down(),
        KeyCode::Char('k') => app.move_cell_up(),
        KeyCode::Char('e') => app.begin_edit_cell(),
        KeyCode::Char('r') => app.reload_current_table(),
        KeyCode::Char('c') => {
            app.copy_current_cell_tsv();
        }
        KeyCode::Char('C') => {
            app.copy_current_row_tsv();
        }
        KeyCode::Char('u') => {
            if let Some(table) = app.current_table_name().map(|s| s.to_string()) {
                let _ = app.req_tx.send(DBRequest::UndoLastChange { table });
                app.status = "Undoing last change...".into();
            } else {
                app.status = "No table selected to undo".into();
            }
        }
        KeyCode::Char('+') | KeyCode::Char('=') => {
            app.resize_current_column_wider();
            app.status = "Column width: wider".into();
        }
        KeyCode::Char('-') | KeyCode::Char('_') => {
            app.resize_current_column_narrower();
            app.status = "Column width: narrower".into();
        }
        KeyCode::Char('a') => {
            app.request_autosize_current_column();
            app.status = "Autosizing current column…".into();
        }
        KeyCode::Char('A') => {
            app.request_autosize_all_columns();
            app.status = "Autosizing all columns…".into();
        }
        KeyCode::Char('v') => {
            app.toggle_cell_viewer();
            if app.show_cell_viewer {
                app.status = "Cell viewer: ON".into();
            } else {
                app.status = "Cell viewer: OFF".into();
            }
        }
        _ => {}
    }
    false
}

fn handle_key_editing(app: &mut App, key: crossterm::event::KeyEvent) -> bool {
    use crossterm::event::{KeyCode::*, KeyModifiers};

    match key.code {
        Enter | Char('\n') | Char('\r') => app.submit_cell_edit(),
        Esc => app.cancel_edit_cell(),
        Backspace => app.edit_input_backspace(),
        Delete => app.edit_input_delete(),
        Left => app.edit_input_left(),
        Right => app.edit_input_right(),
        Home => app.edit_input_home(),
        End => app.edit_input_end(),
        Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.edit_mark_null();
        }
        Char(c) => {
            let c = if key.modifiers.contains(KeyModifiers::CONTROL) {
                // ignore control chars in insert
                '\0'
            } else {
                c
            };
            if c != '\0' {
                app.edit_input_insert(c);
            }
        }
        _ => {}
    }
    false
}
