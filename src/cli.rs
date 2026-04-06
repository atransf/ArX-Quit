use anyhow::{Result, bail};
use clap::{Parser, Subcommand};

use crate::app;
use crate::process::{self, GuiApp};

#[derive(Parser)]
#[command(name = "arxkill", about = "macOS GUI app manager")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// List running GUI applications
    Ls,
    /// Quit a running application
    Kill {
        /// App name (case-insensitive match)
        name: Option<String>,
        /// Force quit (SIGKILL) instead of graceful
        #[arg(short, long)]
        force: bool,
        /// Target all non-protected apps
        #[arg(short, long)]
        all: bool,
    },
}

pub fn run(cmd: Commands) -> Result<()> {
    match cmd {
        Commands::Ls => run_ls(),
        Commands::Kill { name, force, all } => run_kill(name, force, all),
    }
}

fn format_memory(kb: u64) -> String {
    if kb < 1024 {
        format!("{} KB", kb)
    } else if kb < 1_048_576 {
        format!("{:.1} MB", kb as f64 / 1024.0)
    } else {
        format!("{:.1} GB", kb as f64 / 1_048_576.0)
    }
}

fn quit_app(app: &GuiApp, force: bool) -> bool {
    let action = if force { "Force quit" } else { "Quit" };
    let result = if force {
        process::force_quit(app)
    } else {
        process::graceful_quit(app)
    };
    match result {
        Ok(()) => {
            println!("\u{2705} {} {} (PID {})", action, app.name, app.pid);
            true
        }
        Err(e) => {
            eprintln!("\u{274c} {} {} (PID {}): {}", action, app.name, app.pid, e);
            false
        }
    }
}

fn run_ls() -> Result<()> {
    let apps = process::list_gui_apps()?;
    let protected = app::load_protected_apps();

    println!(
        "{:<30} {:>7}  {:<30} {:>10}  {}",
        "NAME", "PID", "BUNDLE ID", "MEMORY", "STATUS"
    );
    println!("{}", "-".repeat(95));

    for app in &apps {
        let name = if protected.contains(&app.name) {
            format!("\u{1f512} {}", app.name)
        } else {
            app.name.clone()
        };
        let status = if app.is_frozen {
            "Not Responding"
        } else {
            "Running"
        };
        println!(
            "{:<30} {:>7}  {:<30} {:>10}  {}",
            name,
            app.pid,
            app.bundle_id,
            format_memory(app.memory_kb),
            status
        );
    }

    Ok(())
}

fn run_kill(name: Option<String>, force: bool, all: bool) -> Result<()> {
    let apps = process::list_gui_apps()?;
    let protected = app::load_protected_apps();

    if all {
        let mut quit_count = 0u32;
        let mut fail_count = 0u32;
        let mut skip_count = 0u32;

        for app in &apps {
            if protected.contains(&app.name) {
                skip_count += 1;
                continue;
            }
            if quit_app(app, force) {
                quit_count += 1;
            } else {
                fail_count += 1;
            }
        }

        println!("\n{} apps quit successfully", quit_count);
        if skip_count > 0 {
            println!("Skipped {} protected apps", skip_count);
        }
        if fail_count > 0 {
            bail!("{} app(s) failed to quit", fail_count);
        }

        return Ok(());
    }

    let query = match name {
        Some(ref q) => q,
        None => bail!("Specify an app name or use --all"),
    };

    let query_lower = query.to_lowercase();
    let matches: Vec<&GuiApp> = apps
        .iter()
        .filter(|app| app.name.to_lowercase().contains(&query_lower))
        .collect();

    if matches.is_empty() {
        bail!("No running app matches \"{}\"", query);
    }

    let exact = matches
        .iter()
        .find(|app| app.name.to_lowercase() == query_lower)
        .copied();

    if matches.len() > 1 && exact.is_none() {
        println!("Multiple apps match \"{}\":", query);
        for app in &matches {
            println!("  - {} (PID {})", app.name, app.pid);
        }
        bail!("Multiple apps match, be more specific");
    }

    let target = exact.unwrap_or(matches[0]);

    if protected.contains(&target.name) {
        bail!("{} is a protected app", target.name);
    }

    if !quit_app(target, force) {
        let action = if force { "force quit" } else { "quit" };
        bail!("Failed to {} {} (PID {})", action, target.name, target.pid);
    }

    Ok(())
}
