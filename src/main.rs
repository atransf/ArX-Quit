mod app;
mod process;
mod ui;

use app::{App, Message};
use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io;
use std::time::{Duration, Instant};

fn main() -> Result<()> {
    // Panic hook to restore terminal
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        disable_raw_mode().ok();
        execute!(io::stderr(), LeaveAlternateScreen).ok();
        original_hook(panic_info);
    }));

    // Terminal setup
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // App state
    let mut app = App::new();
    let tick_rate = Duration::from_secs(5);
    let mut last_tick = Instant::now();

    // Main loop
    while app.running {
        terminal.draw(|frame| ui::draw(frame, &app))?;

        app.clear_stale_status();

        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if event::poll(timeout)?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
            && let Some(msg) = app.handle_key_event(key)
        {
            app.update(msg);
        }

        if last_tick.elapsed() >= tick_rate {
            app.update(Message::RefreshList);
            last_tick = Instant::now();
        }
    }

    // Terminal teardown
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}
