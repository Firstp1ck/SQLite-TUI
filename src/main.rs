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
    loop {
        // Process any DB responses without blocking
        while let Ok(msg) = app.resp_rx.try_recv() {
            app.handle_db_response(msg);
        }

        terminal.draw(|f| ui::draw(f, app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or(Duration::from_secs(0));

        let should_exit = if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                match app.mode {
                    AppMode::Normal => handle_key_normal(app, key.code),
                    AppMode::Editing { .. } => handle_key_editing(app, key),
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

        if last_tick.elapsed() >= tick_rate {
            // periodic tasks if needed
            *last_tick = Instant::now();
        }

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
        KeyCode::Up => app.move_table_selection_up(),
        KeyCode::Down => app.move_table_selection_down(),
        KeyCode::Enter => app.load_selected_table_page(0),
        KeyCode::PageDown => app.next_page(),
        KeyCode::PageUp => app.prev_page(),
        KeyCode::Left => app.move_cell_left(),
        KeyCode::Right => app.move_cell_right(),
        KeyCode::Char('j') => app.move_cell_down(),
        KeyCode::Char('k') => app.move_cell_up(),
        KeyCode::Char('e') => app.begin_edit_cell(),
        KeyCode::Char('r') => app.reload_current_table(),
        _ => {}
    }
    false
}

fn handle_key_editing(app: &mut App, key: crossterm::event::KeyEvent) -> bool {
    use crossterm::event::{KeyCode::*, KeyModifiers};

    match key.code {
        Enter => app.submit_cell_edit(),
        Esc => app.cancel_edit_cell(),
        Backspace => app.edit_input_backspace(),
        Delete => app.edit_input_delete(),
        Left => app.edit_input_left(),
        Right => app.edit_input_right(),
        Home => app.edit_input_home(),
        End => app.edit_input_end(),
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
