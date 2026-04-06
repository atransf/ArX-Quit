use crate::process::{self, GuiApp};
use crossterm::event::{KeyCode, KeyEvent, MouseEvent, MouseEventKind, MouseButton};
use std::collections::HashSet;
use std::time::{Duration, SystemTime};

#[derive(Clone, Copy, PartialEq)]
pub enum SortMode {
    NameAsc,
    NameDesc,
    PidAsc,
    MemDesc,
}

impl SortMode {
    pub fn label(&self) -> &'static str {
        match self {
            SortMode::NameAsc => "\u{2191} Name",
            SortMode::NameDesc => "\u{2193} Name",
            SortMode::PidAsc => "\u{2191} PID",
            SortMode::MemDesc => "\u{2193} Mem",
        }
    }

    fn next(self) -> Self {
        match self {
            SortMode::NameAsc => SortMode::NameDesc,
            SortMode::NameDesc => SortMode::PidAsc,
            SortMode::PidAsc => SortMode::MemDesc,
            SortMode::MemDesc => SortMode::NameAsc,
        }
    }
}

#[derive(serde::Deserialize)]
struct ProtectedConfig {
    protected: Vec<String>,
}

fn load_protected_apps() -> HashSet<String> {
    let mut set: HashSet<String> = [
        "Finder",
        "loginwindow",
        "SystemUIServer",
        "Dock",
        "WindowServer",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    if let Some(home) = std::env::var_os("HOME") {
        let config_path = std::path::Path::new(&home)
            .join(".config")
            .join("arx-quit")
            .join("protected.toml");
        if let Ok(contents) = std::fs::read_to_string(&config_path) {
            if let Ok(config) = toml::from_str::<ProtectedConfig>(&contents) {
                set.extend(config.protected);
            }
        }
    }

    set
}

pub struct App {
    pub apps: Vec<GuiApp>,
    pub selected_index: usize,
    pub selected_pids: HashSet<u32>,
    pub running: bool,
    pub status_message: Option<(String, bool)>,
    pub confirm_dialog: Option<ConfirmDialog>,
    pub status_set_at: Option<std::time::Instant>,
    pub filter_query: String,
    pub filter_active: bool,
    pub sort_mode: SortMode,
    pub group_mode: bool,
    pub show_history: bool,
    pub show_preview: bool,
    pub quit_history: Vec<HistoryEntry>,
    pub protected_apps: HashSet<String>,
    pub last_click: Option<(u16, std::time::Instant)>,
}

pub struct ConfirmDialog {
    pub app_names: Vec<String>,
    pub action: QuitAction,
}

#[derive(Clone, Copy)]
pub enum QuitAction {
    Graceful,
    Force,
}

pub struct HistoryEntry {
    pub timestamp: SystemTime,
    pub app_name: String,
    pub action: QuitAction,
    pub success: bool,
}

pub enum Message {
    MoveUp,
    MoveDown,
    ToggleSelect,
    SelectAll,
    DeselectAll,
    RequestGracefulQuit,
    RequestForceQuit,
    RequestRestart,
    ConfirmYes,
    ConfirmNo,
    RefreshList,
    Quit,
    EnterFilter,
    ExitFilter,
    FilterInput(char),
    FilterBackspace,
    CycleSort,
    ToggleGrouping,
    ToggleHistory,
    TogglePreview,
}

impl App {
    pub fn new() -> Self {
        let apps = process::list_gui_apps().unwrap_or_default();
        let protected_apps = load_protected_apps();
        Self {
            apps,
            selected_index: 0,
            selected_pids: HashSet::new(),
            running: true,
            status_message: None,
            confirm_dialog: None,
            status_set_at: None,
            filter_query: String::new(),
            filter_active: false,
            sort_mode: SortMode::NameAsc,
            group_mode: false,
            show_history: false,
            show_preview: false,
            quit_history: Vec::new(),
            protected_apps,
            last_click: None,
        }
    }

    pub fn is_protected(&self, name: &str) -> bool {
        self.protected_apps.contains(name)
    }

    pub fn filtered_sorted_apps(&self) -> Vec<&GuiApp> {
        let mut result: Vec<&GuiApp> = if self.filter_query.is_empty() {
            self.apps.iter().collect()
        } else {
            let q = self.filter_query.to_lowercase();
            self.apps
                .iter()
                .filter(|a| a.name.to_lowercase().contains(&q))
                .collect()
        };

        match self.sort_mode {
            SortMode::NameAsc => result.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase())),
            SortMode::NameDesc => result.sort_by(|a, b| b.name.to_lowercase().cmp(&a.name.to_lowercase())),
            SortMode::PidAsc => result.sort_by_key(|a| a.pid),
            SortMode::MemDesc => result.sort_by(|a, b| b.memory_kb.cmp(&a.memory_kb)),
        }

        result
    }

    fn target_apps(&self) -> Vec<&GuiApp> {
        let visible = self.filtered_sorted_apps();
        let candidates = if self.selected_pids.is_empty() {
            visible.get(self.selected_index).copied().into_iter().collect::<Vec<_>>()
        } else {
            visible.iter().filter(|a| self.selected_pids.contains(&a.pid)).copied().collect()
        };
        candidates.into_iter().filter(|a| !self.is_protected(&a.name)).collect()
    }

    pub fn handle_mouse_event(&mut self, mouse: MouseEvent) -> Option<Message> {
        match mouse.kind {
            MouseEventKind::ScrollUp => Some(Message::MoveUp),
            MouseEventKind::ScrollDown => Some(Message::MoveDown),
            MouseEventKind::Down(MouseButton::Left) => {
                // List starts at row 9 (8-row header + 1 border),
                // plus 1 more if the filter bar is visible.
                let list_start: u16 = if self.filter_active { 10 } else { 9 };
                let row = mouse.row;
                if row < list_start {
                    return None;
                }
                let app_index = (row - list_start) as usize;
                let visible_len = self.filtered_sorted_apps().len();
                if visible_len == 0 {
                    return None;
                }
                let clamped = app_index.min(visible_len - 1);

                let now = std::time::Instant::now();
                if let Some((last_row, last_time)) = self.last_click {
                    if last_row == row && now.duration_since(last_time) < Duration::from_millis(500) {
                        self.last_click = None;
                        self.selected_index = clamped;
                        return Some(Message::ToggleSelect);
                    }
                }
                self.last_click = Some((row, now));
                self.selected_index = clamped;
                None
            }
            _ => None,
        }
    }

    pub fn update(&mut self, msg: Message) {
        match msg {
            Message::MoveUp => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                }
            }
            Message::MoveDown => {
                let len = self.filtered_sorted_apps().len();
                if len > 0 && self.selected_index < len - 1 {
                    self.selected_index += 1;
                }
            }
            Message::ToggleSelect => {
                let visible = self.filtered_sorted_apps();
                if let Some(app) = visible.get(self.selected_index) {
                    if self.is_protected(&app.name) {
                        self.set_status(format!("Cannot quit protected app: {}", app.name), false);
                        return;
                    }
                    let pid = app.pid;
                    if !self.selected_pids.remove(&pid) {
                        self.selected_pids.insert(pid);
                    }
                }
            }
            Message::SelectAll => {
                let visible = self.filtered_sorted_apps();
                self.selected_pids = visible.iter()
                    .filter(|a| !self.is_protected(&a.name))
                    .map(|a| a.pid)
                    .collect();
            }
            Message::DeselectAll => {
                self.selected_pids.clear();
            }
            Message::RequestGracefulQuit | Message::RequestForceQuit => {
                let targets = self.target_apps();
                if targets.is_empty() {
                    self.warn_if_protected();
                    return;
                }
                let action = if matches!(msg, Message::RequestForceQuit) {
                    QuitAction::Force
                } else {
                    QuitAction::Graceful
                };
                let names: Vec<String> = targets.iter().map(|a| a.name.clone()).collect();
                self.confirm_dialog = Some(ConfirmDialog {
                    app_names: names,
                    action,
                });
            }
            Message::RequestRestart => {
                let targets = self.target_apps();
                if targets.is_empty() {
                    self.warn_if_protected();
                    return;
                }
                let app = targets[0].clone();
                let bundle_id = app.bundle_id.clone();
                let name = app.name.clone();

                if process::graceful_quit(&app).is_ok() {
                    self.set_status(format!("Restarting {}...", name), true);
                    std::thread::spawn(move || {
                        std::thread::sleep(Duration::from_millis(800));
                        process::relaunch(&GuiApp {
                            name,
                            pid: 0,
                            bundle_id,
                            memory_kb: 0,
                            cpu_percent: 0.0,
                            is_frozen: false,
                        });
                    });
                    self.refresh_list();
                } else {
                    self.set_status(format!("Failed to restart {}", name), false);
                }
            }
            Message::ConfirmYes => {
                if let Some(dialog) = self.confirm_dialog.take() {
                    let action_name = match dialog.action {
                        QuitAction::Graceful => "quit",
                        QuitAction::Force => "force quit",
                    };

                    let targets: Vec<GuiApp> = self.target_apps().into_iter().cloned().collect();
                    let mut succeeded = 0usize;
                    let mut failed = 0usize;

                    for app in &targets {
                        let result = match dialog.action {
                            QuitAction::Graceful => process::graceful_quit(app),
                            QuitAction::Force => process::force_quit(app),
                        };
                        let success = result.is_ok();
                        if success { succeeded += 1 } else { failed += 1 }
                        self.quit_history.push(HistoryEntry {
                            timestamp: SystemTime::now(),
                            app_name: app.name.clone(),
                            action: dialog.action,
                            success,
                        });
                        if self.quit_history.len() > 100 {
                            self.quit_history.remove(0);
                        }
                    }

                    let total = targets.len();
                    if total == 1 {
                        if failed == 0 {
                            self.set_status(format!("{} — {} successfully", dialog.app_names[0], action_name), true);
                        } else {
                            self.set_status(format!("Failed to {} {}", action_name, dialog.app_names[0]), false);
                        }
                    } else if failed == 0 {
                        self.set_status(format!("{} apps {} successfully", succeeded, action_name), true);
                    } else {
                        self.set_status(format!("{} succeeded, {} failed to {}", succeeded, failed, action_name), failed == total);
                    }

                    self.selected_pids.clear();
                    self.refresh_list();
                }
            }
            Message::ConfirmNo => {
                self.confirm_dialog = None;
            }
            Message::RefreshList => {
                self.refresh_list();
            }
            Message::Quit => {
                if self.confirm_dialog.is_some() {
                    self.confirm_dialog = None;
                } else {
                    self.running = false;
                }
            }
            Message::EnterFilter => {
                self.filter_active = true;
                self.filter_query.clear();
                self.selected_index = 0;
            }
            Message::ExitFilter => {
                self.filter_active = false;
                self.filter_query.clear();
                self.selected_index = 0;
            }
            Message::FilterInput(c) => {
                self.filter_query.push(c);
                self.selected_index = 0;
            }
            Message::FilterBackspace => {
                self.filter_query.pop();
                self.selected_index = 0;
            }
            Message::CycleSort => {
                self.sort_mode = self.sort_mode.next();
                self.selected_index = 0;
            }
            Message::ToggleGrouping => {
                self.group_mode = !self.group_mode;
                self.selected_index = 0;
            }
            Message::ToggleHistory => {
                self.show_history = !self.show_history;
            }
            Message::TogglePreview => {
                self.show_preview = !self.show_preview;
            }
        }
    }

    pub fn handle_key_event(&self, key: KeyEvent) -> Option<Message> {
        if self.confirm_dialog.is_some() {
            return match key.code {
                KeyCode::Char('y') | KeyCode::Enter => Some(Message::ConfirmYes),
                KeyCode::Char('n') | KeyCode::Esc => Some(Message::ConfirmNo),
                _ => None,
            };
        }

        if self.show_history {
            return match key.code {
                KeyCode::Char('l') | KeyCode::Esc => Some(Message::ToggleHistory),
                _ => None,
            };
        }

        if self.filter_active {
            return match key.code {
                KeyCode::Esc | KeyCode::Char('/') => Some(Message::ExitFilter),
                KeyCode::Backspace => Some(Message::FilterBackspace),
                KeyCode::Char(c) if c != '\n' => Some(Message::FilterInput(c)),
                _ => None,
            };
        }

        match key.code {
            KeyCode::Up | KeyCode::Char('k') => Some(Message::MoveUp),
            KeyCode::Down | KeyCode::Char('j') => Some(Message::MoveDown),
            KeyCode::Char(' ') => Some(Message::ToggleSelect),
            KeyCode::Char('a') => Some(Message::SelectAll),
            KeyCode::Char('d') => Some(Message::DeselectAll),
            KeyCode::Enter | KeyCode::Char('r') => Some(Message::RequestGracefulQuit),
            KeyCode::Char('f') => Some(Message::RequestForceQuit),
            KeyCode::Char('R') => Some(Message::RequestRestart),
            KeyCode::Char('e') => Some(Message::RefreshList),
            KeyCode::Char('q') => Some(Message::Quit),
            KeyCode::Char('/') => Some(Message::EnterFilter),
            KeyCode::Char('s') => Some(Message::CycleSort),
            KeyCode::Char('g') => Some(Message::ToggleGrouping),
            KeyCode::Char('l') => Some(Message::ToggleHistory),
            KeyCode::Tab | KeyCode::Char('p') => Some(Message::TogglePreview),
            KeyCode::Esc => Some(Message::Quit),
            _ => None,
        }
    }

    pub fn clear_stale_status(&mut self) {
        if let Some(set_at) = self.status_set_at
            && set_at.elapsed() > std::time::Duration::from_secs(5)
        {
            self.status_message = None;
            self.status_set_at = None;
        }
    }

    fn warn_if_protected(&mut self) {
        let visible = self.filtered_sorted_apps();
        if let Some(app) = visible.get(self.selected_index) {
            if self.is_protected(&app.name) {
                self.set_status(format!("Cannot quit protected app: {}", app.name), false);
            }
        }
    }

    fn set_status(&mut self, msg: String, success: bool) {
        self.status_message = Some((msg, success));
        self.status_set_at = Some(std::time::Instant::now());
    }

    fn refresh_list(&mut self) {
        if let Ok(apps) = process::list_gui_apps() {
            self.apps = apps;
            let current_pids: HashSet<u32> = self.apps.iter().map(|a| a.pid).collect();
            self.selected_pids.retain(|pid| current_pids.contains(pid));
            let visible_len = self.filtered_sorted_apps().len();
            if self.selected_index >= visible_len && visible_len > 0 {
                self.selected_index = visible_len - 1;
            }
        }
    }
}
