use anyhow::{bail, Context, Result};
use std::collections::HashMap;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct GuiApp {
    pub name: String,
    pub pid: u32,
    pub bundle_id: String,
    pub memory_kb: u64,
    pub cpu_percent: f32,
    pub is_frozen: bool,
}

fn run_applescript(script: &str) -> Result<String> {
    let output = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .context("Failed to run osascript")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("osascript failed: {}", stderr);
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn parse_applescript_list(raw: &str) -> Vec<String> {
    if raw.is_empty() {
        return Vec::new();
    }
    raw.split(", ").map(|s| s.trim().to_string()).collect()
}

/// Batch-fetch RSS (KB) and CPU% for a list of PIDs using a single `ps` call.
fn fetch_resource_usage(pids: &[u32]) -> HashMap<u32, (u64, f32)> {
    let mut map = HashMap::new();
    if pids.is_empty() {
        return map;
    }

    let pid_args: Vec<String> = pids.iter().map(|p| p.to_string()).collect();
    let pid_list = pid_args.join(",");

    let output = Command::new("ps")
        .args(["-o", "pid=,rss=,pcpu=", "-p", &pid_list])
        .output();

    if let Ok(output) = output {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                if let (Ok(pid), Ok(rss), Ok(cpu)) = (
                    parts[0].parse::<u32>(),
                    parts[1].parse::<u64>(),
                    parts[2].parse::<f32>(),
                ) {
                    map.insert(pid, (rss, cpu));
                }
            }
        }
    }

    map
}

pub fn list_gui_apps() -> Result<Vec<GuiApp>> {
    let names_raw = run_applescript(
        "tell application \"System Events\" to get name of every process whose background only is false",
    )?;
    let bundles_raw = run_applescript(
        "tell application \"System Events\" to get bundle identifier of every process whose background only is false",
    )?;
    let pids_raw = run_applescript(
        "tell application \"System Events\" to get unix id of every process whose background only is false",
    )?;

    let names = parse_applescript_list(&names_raw);
    let bundles = parse_applescript_list(&bundles_raw);
    let pids = parse_applescript_list(&pids_raw);

    let my_pid = std::process::id();

    let mut apps: Vec<GuiApp> = names
        .into_iter()
        .zip(bundles)
        .zip(pids)
        .filter_map(|((name, bundle_id), pid_str)| {
            let pid = pid_str.parse::<u32>().ok()?;
            if pid == my_pid {
                return None;
            }
            Some(GuiApp {
                name,
                pid,
                bundle_id,
                memory_kb: 0,
                cpu_percent: 0.0,
                is_frozen: false,
            })
        })
        .collect();

    // Batch fetch resource usage
    let all_pids: Vec<u32> = apps.iter().map(|a| a.pid).collect();
    let usage = fetch_resource_usage(&all_pids);
    for app in &mut apps {
        if let Some(&(rss, cpu)) = usage.get(&app.pid) {
            app.memory_kb = rss;
            app.cpu_percent = cpu;
        }
    }

    // Batch freeze detection
    let responding_raw = run_applescript(
        "tell application \"System Events\" to get responding of every process whose background only is false",
    );
    if let Ok(raw) = responding_raw {
        let states = parse_applescript_list(&raw);
        // Match states to apps by original order (before filtering self)
        // We need to re-derive: the AppleScript returns values for ALL foreground processes,
        // but we filtered out our own PID. We must match by index in the original unfiltered list.
        // Since we filtered during construction, we need a different approach:
        // Build a name->responding map instead.
        // Actually the simplest: the states list corresponds 1:1 to the names/bundles/pids lists
        // which were fetched in the same order. We filtered out our own PID, so we need to
        // track which indices were kept.
        // Let's just iterate states alongside apps (which may have fewer entries due to self-filter).
        // Instead, fetch responding separately with names to correlate.
        let names_for_responding = run_applescript(
            "tell application \"System Events\" to get name of every process whose background only is false",
        );
        if let Ok(names_raw) = names_for_responding {
            let names_list = parse_applescript_list(&names_raw);
            let mut responding_map: HashMap<String, bool> = HashMap::new();
            for (name, state) in names_list.into_iter().zip(states.into_iter()) {
                responding_map.insert(name, state == "true");
            }
            for app in &mut apps {
                if let Some(&responding) = responding_map.get(&app.name) {
                    app.is_frozen = !responding;
                }
            }
        }
    }

    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(apps)
}

pub fn graceful_quit(app: &GuiApp) -> Result<()> {
    let script = format!("tell application \"{}\" to quit", app.name);
    run_applescript(&script)?;
    Ok(())
}

pub fn relaunch(app: &GuiApp) {
    Command::new("open")
        .arg("-b")
        .arg(&app.bundle_id)
        .output()
        .ok();
}

pub fn force_quit(app: &GuiApp) -> Result<()> {
    let output = Command::new("kill")
        .arg("-9")
        .arg(app.pid.to_string())
        .output()
        .context("Failed to execute kill command")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("Force quit failed for {} (PID {}): {}", app.name, app.pid, stderr);
    }

    Ok(())
}
