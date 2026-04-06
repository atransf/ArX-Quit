use anyhow::{Context, Result, bail};
use std::collections::HashMap;
use std::process::Command;
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct GuiApp {
    pub name: String,
    pub pid: u32,
    pub bundle_id: String,
    pub memory_kb: u64,
    pub cpu_percent: f32,
    pub is_frozen: bool,
}

/// Snapshot of per-PID cumulative CPU time (as f64 seconds) at a point in time.
/// Uses `ps -o pid=,cputime=` which outputs `[HH:]MM:SS.ss` on macOS.
#[derive(Clone)]
pub struct CpuSnapshot {
    pub taken_at: Instant,
    pub ticks: HashMap<u32, f64>,
}

impl CpuSnapshot {
    pub fn capture(pids: &[u32]) -> Self {
        let mut ticks = HashMap::new();
        if pids.is_empty() {
            return Self {
                taken_at: Instant::now(),
                ticks,
            };
        }
        let pid_list = pids
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<_>>()
            .join(",");
        if let Ok(output) = Command::new("ps")
            .args(["-o", "pid=,cputime=", "-p", &pid_list])
            .output()
        {
            for line in String::from_utf8_lossy(&output.stdout).lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2
                    && let Ok(pid) = parts[0].parse::<u32>()
                {
                    ticks.insert(pid, parse_cputime(parts[1]));
                }
            }
        }
        Self {
            taken_at: Instant::now(),
            ticks,
        }
    }

    /// Real-time CPU% for each PID: (Δ cpu_seconds / Δ wall_seconds) × 100.
    pub fn delta_cpu(&self, prev: &CpuSnapshot) -> HashMap<u32, f32> {
        let elapsed = self.taken_at.duration_since(prev.taken_at).as_secs_f64();
        if elapsed < 0.1 {
            return HashMap::new();
        }
        let mut out = HashMap::new();
        for (&pid, &cur) in &self.ticks {
            if let Some(&prev_val) = prev.ticks.get(&pid) {
                let delta = (cur - prev_val).max(0.0);
                out.insert(pid, (delta / elapsed * 100.0) as f32);
            }
        }
        out
    }
}

/// Parse macOS `cputime` field `[HH:]MM:SS.ss` into total seconds as f64.
fn parse_cputime(s: &str) -> f64 {
    let parts: Vec<&str> = s.split(':').collect();
    match parts.len() {
        3 => {
            let h = parts[0].parse::<f64>().unwrap_or(0.0);
            let m = parts[1].parse::<f64>().unwrap_or(0.0);
            let sec = parts[2].parse::<f64>().unwrap_or(0.0);
            h * 3600.0 + m * 60.0 + sec
        }
        2 => {
            let m = parts[0].parse::<f64>().unwrap_or(0.0);
            let sec = parts[1].parse::<f64>().unwrap_or(0.0);
            m * 60.0 + sec
        }
        1 => parts[0].parse::<f64>().unwrap_or(0.0),
        _ => 0.0,
    }
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

fn fetch_rss(pids: &[u32]) -> HashMap<u32, u64> {
    let mut map = HashMap::new();
    if pids.is_empty() {
        return map;
    }

    let pid_list = pids
        .iter()
        .map(|p| p.to_string())
        .collect::<Vec<_>>()
        .join(",");

    if let Ok(output) = Command::new("ps")
        .args(["-o", "pid=,rss=", "-p", &pid_list])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2
                && let (Ok(pid), Ok(rss)) = (parts[0].parse::<u32>(), parts[1].parse::<u64>())
            {
                map.insert(pid, rss);
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

    let all_pids: Vec<u32> = apps.iter().map(|a| a.pid).collect();
    let rss_map = fetch_rss(&all_pids);
    for app in &mut apps {
        if let Some(&rss) = rss_map.get(&app.pid) {
            app.memory_kb = rss;
        }
    }

    if let Ok(raw) = run_applescript(
        "tell application \"System Events\" to get responding of every process whose background only is false",
    ) {
        let states = parse_applescript_list(&raw);
        let names_list = parse_applescript_list(&names_raw);
        let responding_map: HashMap<&str, bool> = names_list
            .iter()
            .zip(states.iter())
            .map(|(n, s)| (n.as_str(), s == "true"))
            .collect();
        for app in &mut apps {
            if let Some(&responding) = responding_map.get(app.name.as_str()) {
                app.is_frozen = !responding;
            }
        }
    }

    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(apps)
}

/// Fast CPU + RSS refresh using only `ps` (no AppleScript).
pub fn refresh_cpu_rss(apps: &mut [GuiApp], prev: &CpuSnapshot) -> CpuSnapshot {
    let pids: Vec<u32> = apps.iter().map(|a| a.pid).collect();
    if pids.is_empty() {
        return CpuSnapshot::capture(&[]);
    }
    let new_snap = CpuSnapshot::capture(&pids);
    let cpu_map = new_snap.delta_cpu(prev);
    let rss_map = fetch_rss(&pids);

    for app in apps.iter_mut() {
        if let Some(&rss) = rss_map.get(&app.pid) {
            app.memory_kb = rss;
        }
        if let Some(&pct) = cpu_map.get(&app.pid) {
            app.cpu_percent = pct;
        }
    }

    new_snap
}

pub fn graceful_quit(app: &GuiApp) -> Result<()> {
    let escaped_name = app.name.replace('\\', "\\\\").replace('"', "\\\"");
    let script = format!("tell application \"{}\" to quit", escaped_name);
    Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("Failed to run osascript")?;
    Ok(())
}

pub fn relaunch(bundle_id: &str) {
    Command::new("open").arg("-b").arg(bundle_id).output().ok();
}

pub fn force_quit(app: &GuiApp) -> Result<()> {
    let output = Command::new("kill")
        .arg("-9")
        .arg(app.pid.to_string())
        .output()
        .context("Failed to execute kill command")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "Force quit failed for {} (PID {}): {}",
            app.name,
            app.pid,
            stderr
        );
    }

    Ok(())
}
