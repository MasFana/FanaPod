use crate::podman::{ContainerInfo, ContainerStats, SystemInfo};
use crate::shell::{EnvVar, PathEntry, ShellKind};
use crossterm::event::KeyCode;

#[derive(Clone, Copy, PartialEq)]
pub enum AppMode {
    Dashboard,
    ShellManager,
}

#[derive(Clone, Copy)]
pub enum SortBy {
    Name,
    Status,
    Cpu,
    Memory,
}

#[derive(Clone, Copy, PartialEq)]
pub enum ShellManagerTab {
    Variables,
    Paths,
}

pub enum EditingTarget {
    None,
    AddVarName,
    AddVarValue { name: String },
    AddPath,
    EditVarValue { idx: usize },
    EditVarName { idx: usize },
    EditPath { idx: usize },
}

impl EditingTarget {
    pub fn is_active(&self) -> bool {
        !matches!(self, EditingTarget::None)
    }
}

pub struct ShellManagerState {
    pub env_vars: Vec<EnvVar>,
    pub path_entries: Vec<PathEntry>,
    pub config_path: String,
    pub selected: usize,
    pub selected_shell: ShellKind,
    pub available_shells: Vec<ShellKind>,
    pub active_tab: ShellManagerTab,
    pub editing: EditingTarget,
    pub input_buffer: String,
    pub show_help: bool,
    pub message: Option<String>,
    pub error: Option<String>,
}

pub enum PendingAction {
    Stop(String),
    Delete(String),
}

pub struct App {
    pub mode: AppMode,
    pub containers: Vec<ContainerInfo>,
    pub stats: Vec<ContainerStats>,
    pub system_info: SystemInfo,
    pub selected: usize,
    pub sort_by: SortBy,
    pub error_message: Option<String>,
    pub status_message: Option<String>,
    pub pending_action: Option<PendingAction>,
    pub shell_manager: ShellManagerState,
    pub prev_net_rx: u64,
    pub prev_net_tx: u64,
    pub net_rx_speed: f64,
    pub net_tx_speed: f64,
}

impl App {
    pub fn new() -> Self {
        let shell_manager = ShellManagerState {
            env_vars: vec![],
            path_entries: vec![],
            config_path: String::new(),
            selected: 0,
            selected_shell: ShellKind::Bash,
            available_shells: vec![],
            active_tab: ShellManagerTab::Variables,
            editing: EditingTarget::None,
            input_buffer: String::new(),
            show_help: false,
            message: None,
            error: None,
        };
        match (crate::podman::list_containers(), crate::podman::get_stats()) {
            (Ok(containers), Ok(stats)) => {
                let sys = crate::podman::get_system_info();
                App {
                    mode: AppMode::Dashboard,
                    containers,
                    stats,
                    system_info: sys,
                    selected: 0,
                    sort_by: SortBy::Name,
                    error_message: None,
                    status_message: None,
                    pending_action: None,
                    shell_manager,
                    prev_net_rx: 0,
                    prev_net_tx: 0,
                    net_rx_speed: 0.0,
                    net_tx_speed: 0.0,
                }.sort_initial()
            }
            (Err(e), _) | (_, Err(e)) => {
                let sys = crate::podman::get_system_info();
                App {
                    mode: AppMode::Dashboard,
                    containers: vec![],
                    stats: vec![],
                    system_info: sys,
                    selected: 0,
                    sort_by: SortBy::Name,
                    error_message: Some(e),
                    status_message: None,
                    pending_action: None,
                    shell_manager,
                    prev_net_rx: 0,
                    prev_net_tx: 0,
                    net_rx_speed: 0.0,
                    net_tx_speed: 0.0,
                }.sort_initial()
            }
        }
    }

    pub fn sort_initial(mut self) -> Self {
        self.sort_containers();
        self
    }

    pub fn refresh_stats(&mut self) {
        let new_sys = crate::podman::get_system_info();

        if self.prev_net_rx > 0 || self.prev_net_tx > 0 {
            let rx_delta = new_sys.net_rx_bytes.saturating_sub(self.prev_net_rx);
            let tx_delta = new_sys.net_tx_bytes.saturating_sub(self.prev_net_tx);
            self.net_rx_speed = rx_delta as f64 / 3.0;
            self.net_tx_speed = tx_delta as f64 / 3.0;
        }

        self.prev_net_rx = new_sys.net_rx_bytes;
        self.prev_net_tx = new_sys.net_tx_bytes;
        self.system_info = new_sys;

        match (crate::podman::list_containers(), crate::podman::get_stats()) {
            (Ok(containers), Ok(stats)) => {
                self.containers = containers;
                self.stats = stats;
                self.sort_containers();
                if self.containers.is_empty() {
                    self.selected = 0;
                } else {
                    self.selected = self.selected.min(self.containers.len().saturating_sub(1));
                }
                self.error_message = None;
            }
            (Err(e), _) | (_, Err(e)) => {
                self.error_message = Some(e);
            }
        }
    }

    pub fn sort_containers(&mut self) {
        match self.sort_by {
            SortBy::Memory => {
                self.containers.sort_by(|a, b| {
                    let ma = self
                        .stats
                        .iter()
                        .find(|s| s.name == a.name)
                        .map(|s| s.mem_percent)
                        .unwrap_or(0.0);
                    let mb = self
                        .stats
                        .iter()
                        .find(|s| s.name == b.name)
                        .map(|s| s.mem_percent)
                        .unwrap_or(0.0);
                    mb.partial_cmp(&ma).unwrap_or(std::cmp::Ordering::Equal)
                });
            }
            SortBy::Cpu => {
                self.containers.sort_by(|a, b| {
                    let ca = self
                        .stats
                        .iter()
                        .find(|s| s.name == a.name)
                        .map(|s| s.cpu_percent)
                        .unwrap_or(0.0);
                    let cb = self
                        .stats
                        .iter()
                        .find(|s| s.name == b.name)
                        .map(|s| s.cpu_percent)
                        .unwrap_or(0.0);
                    cb.partial_cmp(&ca).unwrap_or(std::cmp::Ordering::Equal)
                });
            }
            SortBy::Name => {
                self.containers.sort_by(|a, b| a.name.cmp(&b.name));
            }
            SortBy::Status => {
                self.containers.sort_by(|a, b| a.state.cmp(&b.state));
            }
        }
    }

    pub fn switch_mode(&mut self) {
        self.mode = match self.mode {
            AppMode::Dashboard => {
                self.load_shell_config();
                AppMode::ShellManager
            }
            AppMode::ShellManager => AppMode::Dashboard,
        };
    }

    pub fn load_shell_config(&mut self) {
        let available = crate::shell::detect_available_shells();
        let host_shell = crate::shell::detect_host_shell();
        let selected_shell = available
            .iter()
            .find(|&&s| s == host_shell)
            .copied()
            .or_else(|| available.first().copied())
            .unwrap_or(ShellKind::Bash);
        let config = crate::shell::load_shell_config(selected_shell);

        self.shell_manager.available_shells = available;
        self.shell_manager.selected_shell = config.shell;
        self.shell_manager.config_path = config.config_path;
        self.shell_manager.env_vars = config.env_vars;
        self.shell_manager.path_entries = config.path_entries;
        self.shell_manager.selected = 0;
        self.shell_manager.editing = EditingTarget::None;
        self.shell_manager.input_buffer.clear();
    }

    pub fn handle_key(&mut self, key: KeyCode) -> bool {
        if self.mode == AppMode::ShellManager {
            return self.handle_shell_manager_key(key);
        }
        match key {
            KeyCode::Char('v') | KeyCode::Char('m') => {
                self.shell_manager.show_help = false;
                self.switch_mode();
                false
            }
            KeyCode::Char('?') | KeyCode::Char('h') => {
                self.shell_manager.show_help = !self.shell_manager.show_help;
                false
            }
            KeyCode::Char('q') | KeyCode::Char('Q') => true,
            KeyCode::Char('j') | KeyCode::Down => {
                if self.shell_manager.show_help {
                    return false;
                }
                if !self.containers.is_empty() {
                    self.selected =
                        (self.selected + 1).min(self.containers.len().saturating_sub(1));
                }
                false
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if self.shell_manager.show_help {
                    return false;
                }
                self.selected = self.selected.saturating_sub(1);
                false
            }
            KeyCode::Char('s') => {
                if let Some(container) = self.containers.get(self.selected) {
                    let name = container.name.clone();
                    let action_result = crate::podman::start_container(&name);
                    self.refresh_stats();
                    match action_result {
                        Ok(()) => {
                            self.status_message = Some(format!("Container started: {}", name));
                            self.error_message = None;
                        }
                        Err(e) => {
                            self.error_message = Some(e);
                        }
                    }
                }
                false
            }
            KeyCode::Char('x') => {
                if let Some(container) = self.containers.get(self.selected) {
                    self.pending_action = Some(PendingAction::Stop(container.name.clone()));
                }
                false
            }
            KeyCode::Char('d') => {
                if let Some(container) = self.containers.get(self.selected) {
                    self.pending_action = Some(PendingAction::Delete(container.name.clone()));
                }
                false
            }
            KeyCode::Char('y') => {
                if let Some(action) = self.pending_action.take() {
                    let (prefix, result) = match &action {
                        PendingAction::Stop(name) => {
                            let r = crate::podman::stop_container(name);
                            (format!("Container stopped: {}", name), r)
                        }
                        PendingAction::Delete(name) => {
                            let r = crate::podman::rm_container(name);
                            (format!("Container deleted: {}", name), r)
                        }
                    };
                    self.refresh_stats();
                    match result {
                        Ok(()) => {
                            self.status_message = Some(prefix);
                            self.error_message = None;
                        }
                        Err(e) => {
                            self.error_message = Some(e);
                        }
                    }
                }
                false
            }
            KeyCode::Esc => {
                self.pending_action = None;
                self.shell_manager.show_help = false;
                false
            }
            KeyCode::Char('r') => {
                self.refresh_stats();
                self.status_message = Some("Stats refreshed".to_string());
                false
            }
            KeyCode::Char('t') => {
                self.sort_by = match self.sort_by {
                    SortBy::Name => SortBy::Status,
                    SortBy::Status => SortBy::Cpu,
                    SortBy::Cpu => SortBy::Memory,
                    SortBy::Memory => SortBy::Name,
                };
                self.sort_containers();
                false
            }
            KeyCode::Char('e') => {
                if let Some(container) = self.containers.get(self.selected) {
                    let name = container.name.clone();
                    let state = container.state.clone();
                    let action_result = crate::podman::exec_container(&name, &state, false);
                    match action_result {
                        Ok(msg) => {
                            self.status_message = Some(msg);
                            self.error_message = None;
                        }
                        Err(e) => {
                            self.error_message = Some(e);
                        }
                    }
                }
                false
            }
            KeyCode::Char('E') => {
                if let Some(container) = self.containers.get(self.selected) {
                    let name = container.name.clone();
                    let state = container.state.clone();
                    let action_result = crate::podman::exec_container(&name, &state, true);
                    match action_result {
                        Ok(msg) => {
                            self.status_message = Some(msg);
                            self.error_message = None;
                        }
                        Err(e) => {
                            self.error_message = Some(e);
                        }
                    }
                }
                false
            }
            KeyCode::Char('b') => {
                let r = crate::podman::open_terminal_in_bin_dir();
                match r {
                    Ok(()) => {
                        let dir = crate::podman::binary_dir();
                        let dir_name = dir
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| dir.display().to_string());
                        self.status_message = Some(format!("Terminal opened in: {}", dir_name));
                        self.error_message = None;
                    }
                    Err(e) => {
                        self.error_message = Some(e);
                    }
                }
                false
            }
            _ => false,
        }
    }

    fn handle_shell_manager_key(&mut self, key: KeyCode) -> bool {
        match key {
            KeyCode::Char('?') => {
                self.shell_manager.show_help = !self.shell_manager.show_help;
                return false;
            }
            _ => {}
        }

        if self.shell_manager.show_help {
            match key {
                KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q') | KeyCode::Char('Q') => {
                    self.shell_manager.show_help = false;
                }
                _ => {}
            }
            return false;
        }

        if self.shell_manager.editing.is_active() {
            return self.handle_editing_key(key);
        }

        match key {
            KeyCode::Tab => {
                self.shell_manager.active_tab = match self.shell_manager.active_tab {
                    ShellManagerTab::Variables => ShellManagerTab::Paths,
                    ShellManagerTab::Paths => ShellManagerTab::Variables,
                };
                self.shell_manager.selected = 0;
                false
            }
            KeyCode::Char('1') => {
                self.shell_manager.active_tab = ShellManagerTab::Variables;
                self.shell_manager.selected = 0;
                false
            }
            KeyCode::Char('2') => {
                self.shell_manager.active_tab = ShellManagerTab::Paths;
                self.shell_manager.selected = 0;
                false
            }
            KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => {
                self.mode = AppMode::Dashboard;
                false
            }
            KeyCode::Char('v') | KeyCode::Char('m') => {
                self.switch_mode();
                false
            }
            KeyCode::Char('j') | KeyCode::Down => {
                match self.shell_manager.active_tab {
                    ShellManagerTab::Variables => {
                        if !self.shell_manager.env_vars.is_empty() {
                            self.shell_manager.selected = (self.shell_manager.selected + 1)
                                .min(self.shell_manager.env_vars.len().saturating_sub(1));
                        }
                    }
                    ShellManagerTab::Paths => {
                        if !self.shell_manager.path_entries.is_empty() {
                            self.shell_manager.selected = (self.shell_manager.selected + 1)
                                .min(self.shell_manager.path_entries.len().saturating_sub(1));
                        }
                    }
                }
                false
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.shell_manager.selected = self.shell_manager.selected.saturating_sub(1);
                false
            }
            KeyCode::Char('a') => {
                match self.shell_manager.active_tab {
                    ShellManagerTab::Variables => {
                        self.shell_manager.editing = EditingTarget::AddVarName;
                    }
                    ShellManagerTab::Paths => {
                        self.shell_manager.editing = EditingTarget::AddPath;
                    }
                }
                false
            }
            KeyCode::Char('d') => {
                match self.shell_manager.active_tab {
                    ShellManagerTab::Variables => {
                        if !self.shell_manager.env_vars.is_empty() {
                            let idx = self.shell_manager.selected;
                            if idx < self.shell_manager.env_vars.len() {
                                let removed = self.shell_manager.env_vars.remove(idx).name;
                                self.shell_manager.selected = self
                                    .shell_manager
                                    .selected
                                    .min(self.shell_manager.env_vars.len().saturating_sub(1));
                                self.shell_manager.message = Some(format!("Removed '{}'", removed));
                            }
                        }
                    }
                    ShellManagerTab::Paths => {
                        if !self.shell_manager.path_entries.is_empty() {
                            let idx = self.shell_manager.selected;
                            if idx < self.shell_manager.path_entries.len() {
                                let removed = self.shell_manager.path_entries.remove(idx).path;
                                self.shell_manager.selected = self
                                    .shell_manager
                                    .selected
                                    .min(self.shell_manager.path_entries.len().saturating_sub(1));
                                self.shell_manager.message = Some(format!("Removed '{}'", removed));
                            }
                        }
                    }
                }
                false
            }
            KeyCode::Char('e') => {
                match self.shell_manager.active_tab {
                    ShellManagerTab::Variables => {
                        if !self.shell_manager.env_vars.is_empty() {
                            let idx = self.shell_manager.selected;
                            if idx < self.shell_manager.env_vars.len() {
                                self.shell_manager.input_buffer =
                                    self.shell_manager.env_vars[idx].value.clone();
                                self.shell_manager.editing = EditingTarget::EditVarValue { idx };
                            }
                        }
                    }
                    ShellManagerTab::Paths => {
                        if !self.shell_manager.path_entries.is_empty() {
                            let idx = self.shell_manager.selected;
                            if idx < self.shell_manager.path_entries.len() {
                                self.shell_manager.input_buffer =
                                    self.shell_manager.path_entries[idx].path.clone();
                                self.shell_manager.editing = EditingTarget::EditPath { idx };
                            }
                        }
                    }
                }
                false
            }
            KeyCode::Char('r') => {
                if matches!(self.shell_manager.active_tab, ShellManagerTab::Variables) {
                    if !self.shell_manager.env_vars.is_empty() {
                        let idx = self.shell_manager.selected;
                        if idx < self.shell_manager.env_vars.len() {
                            self.shell_manager.input_buffer =
                                self.shell_manager.env_vars[idx].name.clone();
                            self.shell_manager.editing = EditingTarget::EditVarName { idx };
                        }
                    }
                }
                false
            }
            KeyCode::Char('t') | KeyCode::Char('T') => {
                match self.shell_manager.active_tab {
                    ShellManagerTab::Variables => {
                        if !self.shell_manager.env_vars.is_empty() {
                            let idx = self.shell_manager.selected;
                            if idx < self.shell_manager.env_vars.len() {
                                self.shell_manager.env_vars[idx].enabled =
                                    !self.shell_manager.env_vars[idx].enabled;
                            }
                        }
                    }
                    ShellManagerTab::Paths => {
                        if !self.shell_manager.path_entries.is_empty() {
                            let idx = self.shell_manager.selected;
                            if idx < self.shell_manager.path_entries.len() {
                                self.shell_manager.path_entries[idx].enabled =
                                    !self.shell_manager.path_entries[idx].enabled;
                            }
                        }
                    }
                }
                false
            }
            KeyCode::Char('s') => {
                let shell = self.shell_manager.selected_shell;
                let env_vars = self.shell_manager.env_vars.clone();
                let path_entries = self.shell_manager.path_entries.clone();
                match crate::shell::save_shell_config(shell, &env_vars, &path_entries) {
                    Ok(_) => {
                        self.shell_manager.message = Some(format!(
                            "Saved to ~{}",
                            crate::shell::shell_config_filename(shell)
                        ));
                        self.shell_manager.error = None;
                    }
                    Err(e) => {
                        self.shell_manager.error = Some(e);
                        self.shell_manager.message = None;
                    }
                }
                false
            }
            KeyCode::Char('<') | KeyCode::Char(',') => {
                if !self.shell_manager.available_shells.is_empty() {
                    let current_idx = self
                        .shell_manager
                        .available_shells
                        .iter()
                        .position(|&s| s == self.shell_manager.selected_shell)
                        .unwrap_or(0);
                    let new_idx = if current_idx == 0 {
                        self.shell_manager.available_shells.len() - 1
                    } else {
                        current_idx - 1
                    };
                    self.switch_shell(self.shell_manager.available_shells[new_idx]);
                }
                false
            }
            KeyCode::Char('>') | KeyCode::Char('.') => {
                if !self.shell_manager.available_shells.is_empty() {
                    let current_idx = self
                        .shell_manager
                        .available_shells
                        .iter()
                        .position(|&s| s == self.shell_manager.selected_shell)
                        .unwrap_or(0);
                    let new_idx = (current_idx + 1) % self.shell_manager.available_shells.len();
                    self.switch_shell(self.shell_manager.available_shells[new_idx]);
                }
                false
            }
            _ => false,
        }
    }

    fn switch_shell(&mut self, new_shell: ShellKind) {
        let config = crate::shell::load_shell_config(new_shell);
        self.shell_manager.selected_shell = config.shell;
        self.shell_manager.config_path = config.config_path;
        self.shell_manager.env_vars = config.env_vars;
        self.shell_manager.path_entries = config.path_entries;
        self.shell_manager.selected = 0;
        self.shell_manager.editing = EditingTarget::None;
        self.shell_manager.input_buffer.clear();
        self.shell_manager.message = Some(format!("Switched to {}", new_shell));
    }

    fn handle_editing_key(&mut self, key: KeyCode) -> bool {
        match key {
            KeyCode::Esc => {
                self.shell_manager.editing = EditingTarget::None;
                self.shell_manager.input_buffer.clear();
                false
            }
            KeyCode::Enter => {
                let editing =
                    std::mem::replace(&mut self.shell_manager.editing, EditingTarget::None);
                let input = std::mem::take(&mut self.shell_manager.input_buffer);
                match editing {
                    EditingTarget::AddVarName => {
                        if input.trim().is_empty() {
                            self.shell_manager.error = Some("Name cannot be empty".to_string());
                            self.shell_manager.editing = EditingTarget::AddVarName;
                            self.shell_manager.input_buffer = input;
                            return false;
                        }
                        self.shell_manager.editing = EditingTarget::AddVarValue {
                            name: input.trim().to_string(),
                        };
                    }
                    EditingTarget::AddVarValue { name } => {
                        let idx = self.shell_manager.env_vars.len();
                        self.shell_manager.env_vars.push(EnvVar {
                            name,
                            value: input,
                            enabled: true,
                        });
                        self.shell_manager.selected = idx;
                        self.shell_manager.message = Some("Variable added".to_string());
                    }
                    EditingTarget::AddPath => {
                        if input.trim().is_empty() {
                            self.shell_manager.error = Some("Path cannot be empty".to_string());
                            self.shell_manager.editing = EditingTarget::AddPath;
                            self.shell_manager.input_buffer = input;
                            return false;
                        }
                        self.shell_manager.path_entries.push(PathEntry {
                            path: input.trim().to_string(),
                            enabled: true,
                            label: crate::shell::guess_path_label(&input),
                        });
                        self.shell_manager.selected =
                            self.shell_manager.path_entries.len().saturating_sub(1);
                        self.shell_manager.message = Some("PATH entry added".to_string());
                    }
                    EditingTarget::EditVarValue { idx } => {
                        if let Some(var) = self.shell_manager.env_vars.get_mut(idx) {
                            var.value = input;
                            self.shell_manager.message = Some(format!("Updated '{}'", var.name));
                        }
                    }
                    EditingTarget::EditVarName { idx } => {
                        if input.trim().is_empty() {
                            self.shell_manager.error = Some("Name cannot be empty".to_string());
                            self.shell_manager.editing = EditingTarget::EditVarName { idx };
                            self.shell_manager.input_buffer = input;
                            return false;
                        }
                        if let Some(var) = self.shell_manager.env_vars.get_mut(idx) {
                            let old = var.name.clone();
                            var.name = input.trim().to_string();
                            self.shell_manager.message =
                                Some(format!("Renamed '{}' -> '{}'", old, var.name));
                        }
                    }
                    EditingTarget::EditPath { idx } => {
                        if input.trim().is_empty() {
                            self.shell_manager.error = Some("Path cannot be empty".to_string());
                            self.shell_manager.editing = EditingTarget::EditPath { idx };
                            self.shell_manager.input_buffer = input;
                            return false;
                        }
                        if let Some(entry) = self.shell_manager.path_entries.get_mut(idx) {
                            entry.path = input.trim().to_string();
                            entry.label = crate::shell::guess_path_label(&entry.path);
                            self.shell_manager.message = Some("PATH entry updated".to_string());
                        }
                    }
                    EditingTarget::None => {}
                }
                self.shell_manager.error = None;
                false
            }
            KeyCode::Backspace => {
                self.shell_manager.input_buffer.pop();
                self.shell_manager.error = None;
                false
            }
            KeyCode::Char(c) => {
                self.shell_manager.input_buffer.push(c);
                self.shell_manager.error = None;
                false
            }
            _ => false,
        }
    }

    pub fn modal_display(&self) -> Option<(String, String, String, String)> {
        if !self.shell_manager.editing.is_active() {
            return None;
        }
        match &self.shell_manager.editing {
            EditingTarget::AddVarName => Some((
                "Add Variable".into(),
                "Name".into(),
                self.shell_manager.input_buffer.clone(),
                "e.g. GOPATH, NPM_CONFIG_PREFIX".into(),
            )),
            EditingTarget::AddVarValue { name } => Some((
                "Add Variable".into(),
                name.clone(),
                self.shell_manager.input_buffer.clone(),
                "e.g. /home/dev/go".into(),
            )),
            EditingTarget::AddPath => Some((
                "Add PATH Entry".into(),
                "Path".into(),
                self.shell_manager.input_buffer.clone(),
                "e.g. /usr/local/go/bin".into(),
            )),
            EditingTarget::EditVarValue { idx } => {
                let var = &self.shell_manager.env_vars[*idx];
                Some((
                    "Edit Variable".into(),
                    var.name.clone(),
                    self.shell_manager.input_buffer.clone(),
                    "Enter new value".into(),
                ))
            }
            EditingTarget::EditVarName { idx } => {
                let var = &self.shell_manager.env_vars[*idx];
                Some((
                    "Rename Variable".into(),
                    var.name.clone(),
                    self.shell_manager.input_buffer.clone(),
                    "Enter new name".into(),
                ))
            }
            EditingTarget::EditPath { idx } => {
                let entry = &self.shell_manager.path_entries[*idx];
                Some((
                    "Edit PATH Entry".into(),
                    entry.path.clone(),
                    self.shell_manager.input_buffer.clone(),
                    "Enter new path".into(),
                ))
            }
            EditingTarget::None => None,
        }
    }

    pub fn keybindings_help() -> Vec<(&'static str, &'static str)> {
        vec![
            ("Navigation", ""),
            ("j / Down", "Move down"),
            ("k / Up", "Move up"),
            ("Tab", "Switch tab (Vars <-> Paths)"),
            ("1 / 2", "Jump to Vars / Paths tab"),
            ("", ""),
            ("Actions", ""),
            ("a", "Add new entry"),
            ("d", "Delete selected entry"),
            ("e", "Edit value / path"),
            ("r", "Rename variable (Vars tab)"),
            ("t", "Toggle enabled/disabled"),
            ("", ""),
            ("Shell Management", ""),
            ("s", "Save config to file"),
            ("< / >", "Switch shell (bash/zsh/fish)"),
            ("? / h", "Toggle this help"),
            ("", ""),
            ("During Input", ""),
            ("Enter", "Confirm / Next field"),
            ("Esc", "Cancel"),
            ("Backspace", "Delete char"),
            ("", ""),
            ("Global", ""),
            ("v / m", "Switch Dashboard <-> Shell Mgr"),
            ("q / Esc", "Quit / Go back"),
        ]
    }
}
