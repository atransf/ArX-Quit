mod app;
mod cli;
mod process;
mod ui;

use anyhow::Result;
use app::App;
use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::io;
use std::sync::mpsc;
use std::time::{Duration, Instant};

enum BgResult {
    AppList(Vec<process::GuiApp>),
    CpuRss(Vec<process::GuiApp>, process::CpuSnapshot),
}

fn main() -> Result<()> {
    let cli = cli::Cli::parse();

    if let Some(cmd) = cli.command {
        return cli::run(cmd);
    }

    run_tui()
}

fn run_tui() -> Result<()> {
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

    let (tx, rx) = mpsc::channel::<BgResult>();

    let poll_interval = Duration::from_millis(16);

    while app.running {
        terminal.draw(|frame| ui::draw(frame, &mut app))?;

        app.clear_stale_status();

        while let Ok(result) = rx.try_recv() {
            match result {
                BgResult::AppList(apps) => {
                    app.apply_app_list(apps);
                }
                BgResult::CpuRss(updated_apps, new_snap) => {
                    app.apply_cpu_rss(updated_apps, new_snap);
                }
            }
        }

        if last_list.elapsed() >= list_tick {
            let tx = tx.clone();
            std::thread::spawn(move || {
                if let Ok(apps) = process::list_gui_apps() {
                    tx.send(BgResult::AppList(apps)).ok();
                }
            });
            last_list = Instant::now();
        }

        if last_cpu.elapsed() >= cpu_tick {
            if let Some(prev_snap) = app.take_cpu_snapshot() {
                let mut apps_clone = app.apps.clone();
                let tx = tx.clone();
                std::thread::spawn(move || {
                    let new_snap = process::refresh_cpu_rss(&mut apps_clone, &prev_snap);
                    tx.send(BgResult::CpuRss(apps_clone, new_snap)).ok();
                });
            }
            last_cpu = Instant::now();
        }

        if event::poll(poll_interval)? {
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
    }

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}
