use crate::process::{self, GuiApp};
use crossterm::event::{KeyCode, KeyEvent};
use std::collections::HashSet;

pub struct App {
    pub apps: Vec<GuiApp>,
    pub selected_index: usize,
    pub selected_pids: HashSet<u32>, // multi-select set
    pub running: bool,
    pub status_message: Option<(String, bool)>, // (message, is_success)
    pub confirm_dialog: Option<ConfirmDialog>,
    pub status_set_at: Option<std::time::Instant>,
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

pub enum Message {
    MoveUp,
    MoveDown,
    ToggleSelect,
    SelectAll,
    DeselectAll,
    RequestGracefulQuit,
    RequestForceQuit,
    ConfirmYes,
    ConfirmNo,
    RefreshList,
    Quit,
}

impl App {
    pub fn new() -> Self {
        let apps = process::list_gui_apps().unwrap_or_default();
        Self {
            apps,
            selected_index: 0,
            selected_pids: HashSet::new(),
            running: true,
            status_message: None,
            confirm_dialog: None,
            status_set_at: None,
        }
    }

    /// Returns the apps targeted by the current action:
    /// all selected apps if any are selected, otherwise just the cursor app.
    fn target_apps(&self) -> Vec<&GuiApp> {
        if self.selected_pids.is_empty() {
            self.apps.get(self.selected_index).into_iter().collect()
        } else {
            self.apps.iter().filter(|a| self.selected_pids.contains(&a.pid)).collect()
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
                if !self.apps.is_empty() && self.selected_index < self.apps.len() - 1 {
                    self.selected_index += 1;
                }
            }
            Message::ToggleSelect => {
                if let Some(app) = self.apps.get(self.selected_index) {
                    let pid = app.pid;
                    if !self.selected_pids.remove(&pid) {
                        self.selected_pids.insert(pid);
                    }
                }
            }
            Message::SelectAll => {
                self.selected_pids = self.apps.iter().map(|a| a.pid).collect();
            }
            Message::DeselectAll => {
                self.selected_pids.clear();
            }
            Message::RequestGracefulQuit => {
                let targets = self.target_apps();
                if !targets.is_empty() {
                    let names: Vec<String> = targets.iter().map(|a| a.name.clone()).collect();
                    self.confirm_dialog = Some(ConfirmDialog {
                        app_names: names,
                        action: QuitAction::Graceful,
                    });
                }
            }
            Message::RequestForceQuit => {
                let targets = self.target_apps();
                if !targets.is_empty() {
                    let names: Vec<String> = targets.iter().map(|a| a.name.clone()).collect();
                    self.confirm_dialog = Some(ConfirmDialog {
                        app_names: names,
                        action: QuitAction::Force,
                    });
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
                        match result {
                            Ok(()) => succeeded += 1,
                            Err(_) => failed += 1,
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
        }
    }

    pub fn handle_key_event(&self, key: KeyEvent) -> Option<Message> {
        if self.confirm_dialog.is_some() {
            match key.code {
                KeyCode::Char('y') | KeyCode::Enter => Some(Message::ConfirmYes),
                KeyCode::Char('n') | KeyCode::Esc => Some(Message::ConfirmNo),
                _ => None,
            }
        } else {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => Some(Message::MoveUp),
                KeyCode::Down | KeyCode::Char('j') => Some(Message::MoveDown),
                KeyCode::Char(' ') => Some(Message::ToggleSelect),
                KeyCode::Char('a') => Some(Message::SelectAll),
                KeyCode::Char('d') => Some(Message::DeselectAll),
                KeyCode::Enter | KeyCode::Char('r') => Some(Message::RequestGracefulQuit),
                KeyCode::Char('f') => Some(Message::RequestForceQuit),
                KeyCode::Char('R') => Some(Message::RefreshList),
                KeyCode::Char('q') => Some(Message::Quit),
                _ => None,
            }
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

    fn set_status(&mut self, msg: String, success: bool) {
        self.status_message = Some((msg, success));
        self.status_set_at = Some(std::time::Instant::now());
    }

    fn refresh_list(&mut self) {
        if let Ok(apps) = process::list_gui_apps() {
            self.apps = apps;
            // Remove selections for apps that no longer exist
            let current_pids: HashSet<u32> = self.apps.iter().map(|a| a.pid).collect();
            self.selected_pids.retain(|pid| current_pids.contains(pid));
            if self.selected_index >= self.apps.len() && !self.apps.is_empty() {
                self.selected_index = self.apps.len() - 1;
            }
        }
    }
}
