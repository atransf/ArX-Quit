mod app;
mod process;
mod ui;

use app::{App, Message};
use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyEventKind, EnableMouseCapture, DisableMouseCapture},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io;
use std::time::{Duration, Instant};

fn main() -> Result<()> {
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        disable_raw_mode().ok();
        execute!(io::stderr(), LeaveAlternateScreen, DisableMouseCapture).ok();
        original_hook(panic_info);
    }));

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    let cpu_tick = Duration::from_secs(1);
    let list_tick = Duration::from_secs(5);
    let mut last_cpu = Instant::now();
    let mut last_list = Instant::now();

    while app.running {
        terminal.draw(|frame| ui::draw(frame, &app))?;

        app.clear_stale_status();

        let cpu_due = cpu_tick.saturating_sub(last_cpu.elapsed());
        let list_due = list_tick.saturating_sub(last_list.elapsed());
        let timeout = cpu_due.min(list_due);

        if event::poll(timeout)? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    if let Some(msg) = app.handle_key_event(key) {
                        app.update(msg);
                    }
                }
                Event::Mouse(mouse) => {
                    if let Some(msg) = app.handle_mouse_event(mouse) {
                        app.update(msg);
                    }
                }
                _ => {}
            }
        }

        if last_cpu.elapsed() >= cpu_tick {
            app.update(Message::RefreshCpu);
            last_cpu = Instant::now();
        }
        if last_list.elapsed() >= list_tick {
            app.update(Message::RefreshList);
            last_list = Instant::now();
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    Ok(())
}
