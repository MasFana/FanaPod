use std::env;
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

// ─── Shell Kinds ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ShellKind {
    Bash,
    Zsh,
    Fish,
}

impl fmt::Display for ShellKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ShellKind::Bash => write!(f, "bash"),
            ShellKind::Zsh => write!(f, "zsh"),
            ShellKind::Fish => write!(f, "fish"),
        }
    }
}

// ─── Environment Variables ────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct EnvVar {
    pub name: String,
    pub value: String,
    pub enabled: bool,
}

// ─── PATH Entries ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct PathEntry {
    pub path: String,
    pub enabled: bool,
    /// Human-friendly label (optional)
    pub label: Option<String>,
}

// ─── Shell Config ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ShellConfig {
    pub shell: ShellKind,
    pub config_path: String,
    pub env_vars: Vec<EnvVar>,
    pub path_entries: Vec<PathEntry>,
}

// ─── Shell Detection ──────────────────────────────────────────────────────────

pub fn detect_shell_kind(shell_path: &str) -> ShellKind {
    let basename = shell_path.rsplit('/').next().unwrap_or(shell_path);
    match basename {
        "zsh" => ShellKind::Zsh,
        "fish" => ShellKind::Fish,
        _ => ShellKind::Bash,
    }
}

pub fn shell_config_filename(shell: ShellKind) -> &'static str {
    match shell {
        ShellKind::Bash => ".bashrc",
        ShellKind::Zsh => ".zshrc",
        ShellKind::Fish => ".config/fish/config.fish",
    }
}

pub fn shell_export_syntax(shell: ShellKind) -> (&'static str, &'static str) {
    match shell {
        ShellKind::Fish => ("set -gx ", ""),
        _ => ("export ", ""),
    }
}

/// Detect which shells are installed on the HOST system
pub fn detect_available_shells() -> Vec<ShellKind> {
    let candidates = [
        ("fish", ShellKind::Fish),
        ("zsh", ShellKind::Zsh),
        ("bash", ShellKind::Bash),
    ];

    candidates
        .iter()
        .filter_map(|&(name, kind)| {
            Command::new("which")
                .arg(name)
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|_| kind)
        })
        .collect()
}

/// Detect the shell the HOST terminal is actually running
pub fn detect_host_shell() -> ShellKind {
    // Priority 1: $SHELL env var
    if let Ok(shell_path) = env::var("SHELL") {
        let kind = detect_shell_kind(&shell_path);
        // Verify it actually exists
        if shell_exists(kind) {
            return kind;
        }
    }

    // Priority 2: Check /proc/$$/comm (Linux)
    if let Ok(comm) = fs::read_to_string(format!("/proc/{}/comm", std::process::id())) {
        let shell_name = comm.trim();
        match shell_name {
            "zsh" => return ShellKind::Zsh,
            "fish" => return ShellKind::Fish,
            "bash" => return ShellKind::Bash,
            _ => {}
        }
    }

    // Priority 3: Check $0 via a subshell
    if let Ok(output) = Command::new("sh")
        .arg("-c")
        .arg("echo $0")
        .output()
    {
        if output.status.success() {
            let shell_name = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let shell_name = shell_name.trim_start_matches('-'); // login shells have - prefix
            match shell_name {
                s if s.contains("zsh") => return ShellKind::Zsh,
                s if s.contains("fish") => return ShellKind::Fish,
                s if s.contains("bash") => return ShellKind::Bash,
                _ => {}
            }
        }
    }

    // Priority 4: Fallback - check which shells are available
    for &(name, kind) in &[("fish", ShellKind::Fish), ("zsh", ShellKind::Zsh)] {
        if Command::new("which")
            .arg(name)
            .output()
            .ok()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return kind;
        }
    }

    // Ultimate fallback
    ShellKind::Bash
}

fn shell_exists(shell: ShellKind) -> bool {
    let name = match shell {
        ShellKind::Fish => "fish",
        ShellKind::Zsh => "zsh",
        ShellKind::Bash => "bash",
    };
    Command::new("which")
        .arg(name)
        .output()
        .ok()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Detect the terminal emulator running on the host
pub fn detect_host_terminal() -> Option<(String, Vec<String>)> {
    // Priority 1: Check $TERM_PROGRAM (set by many terminals)
    if let Ok(term_program) = env::var("TERM_PROGRAM") {
        let terminal = match term_program.to_lowercase().as_str() {
            "apple_terminal" => return Some(("open".to_string(), vec![])), // macOS Terminal
            "iterm.app" => return Some(("open".to_string(), vec!["-a".to_string(), "iTerm".to_string()])),
            "ghostty" => ("ghostty".to_string(), vec!["-e".to_string()]),
            "vscode" => return None, // VSCode integrated terminal
            "wezterm" => ("wezterm".to_string(), vec!["start".to_string(), "--".to_string()]),
            _ => {
                // Try to find the binary
                if Command::new(&term_program)
                    .arg("--version")
                    .output()
                    .is_ok()
                {
                    (term_program, vec!["-e".to_string()])
                } else {
                    return None;
                }
            }
        };
        return Some(terminal);
    }

    // Priority 2: Check $TERM (xterm-256color, screen, tmux, etc.)
    if let Ok(term) = env::var("TERM") {
        if term.contains("tmux") {
            // Inside tmux - use tmux split or just return the command
            return Some(("tmux".to_string(), vec!["split-window".to_string(), "-h".to_string()]));
        }
    }

    // Priority 3: Check $COLORTERM
    if let Ok(colorterm) = env::var("COLORTERM") {
        if colorterm.contains("truecolor") || colorterm.contains("24bit") {
            // Could be many modern terminals - try common ones
        }
    }

    // Priority 4: Scan known terminal emulators
    let candidates: [(&str, &[&str]); 7] = [
        ("ghostty", &["-e"]),
        ("alacritty", &["-e"]),
        ("kitty", &[]),
        ("wezterm", &["start", "--"]),
        ("konsole", &["-e"]),
        ("gnome-terminal", &["--"]),
        ("xterm", &["-e"]),
    ];

    for (term, args) in candidates {
        if Command::new(term)
            .arg("--version")
            .output()
            .is_ok()
            || Command::new(term).arg("-h").output().is_ok()
        {
            return Some((term.to_string(), args.iter().map(|s| s.to_string()).collect()));
        }
    }

    None
}

// ─── Config File Paths ────────────────────────────────────────────────────────

pub fn shell_config_full_path(shell: ShellKind) -> Result<PathBuf, String> {
    let home = env::var("HOME").map_err(|_| "$HOME not set".to_string())?;
    Ok(PathBuf::from(home).join(shell_config_filename(shell)))
}

// ─── Reading Config ───────────────────────────────────────────────────────────

pub fn read_host_shell_config(shell: ShellKind) -> Result<String, String> {
    let path = shell_config_full_path(shell)?;
    fs::read_to_string(&path).map_err(|e| format!("cannot read {}: {}", path.display(), e))
}

pub fn load_shell_config(shell: ShellKind) -> ShellConfig {
    let config_path = shell_config_filename(shell).to_string();
    let config_content = read_host_shell_config(shell).ok();

    let (env_vars, path_entries) = match &config_content {
        Some(content) => parse_shell_config(shell, content),
        None => (Vec::new(), Vec::new()),
    };

    // If no path entries were parsed, load current system PATH
    let path_entries = if path_entries.is_empty() {
        load_current_path_entries()
    } else {
        path_entries
    };

    ShellConfig {
        shell,
        config_path,
        env_vars,
        path_entries,
    }
}

fn load_current_path_entries() -> Vec<PathEntry> {
    env::var("PATH")
        .unwrap_or_default()
        .split(':')
        .filter(|s| !s.is_empty())
        .map(|p| PathEntry {
            path: p.to_string(),
            enabled: true,
            label: guess_path_label(p),
        })
        .collect()
}

pub fn guess_path_label(path: &str) -> Option<String> {
    // Try to infer what a PATH entry is for
    if path.contains("go") || path.contains("golang") {
        Some("Go".to_string())
    } else if path.contains("node") || path.contains("npm") || path.contains("nvm") {
        Some("Node.js".to_string())
    } else if path.contains("python") || path.contains("pyenv") {
        Some("Python".to_string())
    } else if path.contains("rust") || path.contains("cargo") || path.contains(".cargo") {
        Some("Rust/Cargo".to_string())
    } else if path.contains("bun") {
        Some("Bun".to_string())
    } else if path.contains("pnpm") {
        Some("pnpm".to_string())
    } else if path.contains("yarn") {
        Some("Yarn".to_string())
    } else if path.contains("docker") || path.contains("podman") {
        Some("Container Tools".to_string())
    } else if path == "/usr/local/bin" || path == "/usr/bin" || path == "/bin" || path == "/usr/sbin" || path == "/sbin" {
        Some("System".to_string())
    } else if path.contains("homebrew") || path.contains("Homebrew") {
        Some("Homebrew".to_string())
    } else {
        None
    }
}

// ─── Parsing Config ───────────────────────────────────────────────────────────

pub fn parse_shell_config(shell: ShellKind, config_content: &str) -> (Vec<EnvVar>, Vec<PathEntry>) {
    let mut env_vars = Vec::new();
    let mut path_entries = Vec::new();

    let mut _in_podman_tui_section = false;

    for line in config_content.lines() {
        let trimmed = line.trim();

        // Track our managed section
        if trimmed.contains("# === Podman TUI Managed ===") {
            _in_podman_tui_section = true;
            continue;
        }
        if trimmed.contains("# === End Podman TUI Managed ===") {
            _in_podman_tui_section = false;
            continue;
        }

        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }

        match shell {
            ShellKind::Fish => {
                // Parse set -gx VAR VALUE
                if let Some(rest) = trimmed.strip_prefix("set -gx ") {
                    if let Some(eq_pos) = rest.find(' ') {
                        let name = rest[..eq_pos].to_string();
                        let value = rest[eq_pos + 1..].trim().to_string();

                        if name == "PATH" || name == "path" {
                            // Fish PATH is space-separated
                            for p in value.split_whitespace() {
                                path_entries.push(PathEntry {
                                    path: p.to_string(),
                                    enabled: true,
                                    label: guess_path_label(p),
                                });
                            }
                        } else {
                            env_vars.push(EnvVar {
                                name,
                                value,
                                enabled: true,
                            });
                        }
                    }
                }
                // Parse fish_add_path
                else if let Some(rest) = trimmed.strip_prefix("fish_add_path ") {
                    let p = rest.trim().to_string();
                    path_entries.push(PathEntry {
                        path: p,
                        enabled: true,
                        label: guess_path_label(rest.trim()),
                    });
                }
            }
            _ => {
                // Parse export VAR=VALUE
                if let Some(rest) = trimmed.strip_prefix("export ") {
                    if let Some(eq_pos) = rest.find('=') {
                        let name = rest[..eq_pos].to_string();
                        let raw_value = rest[eq_pos + 1..].trim();
                        let value = strip_quotes(raw_value);

                        if name == "PATH" {
                            // PATH entries are colon-separated
                            for p in value.split(':') {
                                if !p.is_empty() {
                                    path_entries.push(PathEntry {
                                        path: p.to_string(),
                                        enabled: true,
                                        label: guess_path_label(p),
                                    });
                                }
                            }
                        } else {
                            env_vars.push(EnvVar {
                                name,
                                value,
                                enabled: true,
                            });
                        }
                    } else if rest == "PATH" {
                        // export PATH="$HOME/bin:$PATH" style - handled differently
                        // This is a PATH append, parse it
                    }
                }
                // Parse PATH="$something:/new/path:$PATH"
                else if trimmed.starts_with("PATH=") || trimmed.starts_with("export PATH=") {
                    let value = if trimmed.starts_with("export PATH=") {
                        &trimmed["export PATH=".len()..]
                    } else {
                        &trimmed["PATH=".len()..]
                    };
                    let value = strip_quotes(value);
                    // Extract literal paths (skip variable references like $PATH)
                    for p in value.split(':') {
                        let p = p.trim();
                        if !p.is_empty() && !p.starts_with('$') {
                            path_entries.push(PathEntry {
                                path: p.to_string(),
                                enabled: true,
                                label: guess_path_label(p),
                            });
                        }
                    }
                }
            }
        }
    }

    (env_vars, path_entries)
}

fn strip_quotes(s: &str) -> String {
    let s = s.trim();
    if s.len() >= 2 {
        if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
            return s[1..s.len() - 1].to_string();
        }
    }
    s.to_string()
}

// ─── Saving Config ────────────────────────────────────────────────────────────

pub fn save_shell_config(
    shell: ShellKind,
    env_vars: &[EnvVar],
    path_entries: &[PathEntry],
) -> Result<String, String> {
    let config_path = shell_config_full_path(shell)?;

    // Ensure parent directory exists (for fish: ~/.config/fish/)
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create config directory: {}", e))?;
    }

    // Read existing content
    let existing = fs::read_to_string(&config_path).unwrap_or_default();

    // Remove any existing Podman TUI managed section
    let content_without_managed = remove_managed_section(&existing);

    // Build the new managed section
    let managed_section = build_managed_section(shell, env_vars, path_entries);

    // Write back
    let new_content = if content_without_managed.is_empty() {
        managed_section
    } else {
        format!("{}\n\n{}", content_without_managed.trim_end(), managed_section)
    };

    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&config_path)
        .map_err(|e| format!("cannot write to {}: {}", config_path.display(), e))?;

    file.write_all(new_content.as_bytes())
        .map_err(|e| format!("failed to write config: {}", e))?;

    Ok(config_path.display().to_string())
}

fn remove_managed_section(content: &str) -> String {
    let mut result = String::new();
    let mut skip = false;

    for line in content.lines() {
        if line.trim().contains("# === Podman TUI Managed ===") {
            skip = true;
            continue;
        }
        if line.trim().contains("# === End Podman TUI Managed ===") {
            skip = false;
            continue;
        }
        if !skip {
            if !result.is_empty() {
                result.push('\n');
            }
            result.push_str(line);
        }
    }

    result
}

fn build_managed_section(shell: ShellKind, env_vars: &[EnvVar], path_entries: &[PathEntry]) -> String {
    let _export_prefix = shell_export_syntax(shell);
    let mut lines = Vec::new();

    lines.push("# === Podman TUI Managed ===".to_string());
    lines.push("# DO NOT EDIT MANUALLY - Changes will be overwritten".to_string());
    lines.push(String::new());

    // PATH section
    if !path_entries.is_empty() {
        lines.push("# PATH entries (managed by Podman TUI)".to_string());
        match shell {
            ShellKind::Fish => {
                // Fish: use fish_add_path for each
                for entry in path_entries.iter().filter(|e| e.enabled) {
                    lines.push(format!("fish_add_path {}", entry.path));
                }
            }
            _ => {
                // Bash/Zsh: build PATH string
                let enabled_paths: Vec<&str> = path_entries
                    .iter()
                    .filter(|e| e.enabled)
                    .map(|e| e.path.as_str())
                    .collect();
                if !enabled_paths.is_empty() {
                    lines.push(format!(
                        "export PATH=\"{}\"",
                        enabled_paths.join(":")
                    ));
                }
            }
        }
        lines.push(String::new());
    }

    // Environment variables section
    if !env_vars.is_empty() {
        lines.push("# Environment variables (managed by Podman TUI)".to_string());
        for var in env_vars.iter().filter(|v| v.enabled) {
            match shell {
                ShellKind::Fish => {
                    if var.value.contains(' ') {
                        lines.push(format!("set -gx {} \"{}\"", var.name, var.value));
                    } else {
                        lines.push(format!("set -gx {} {}", var.name, var.value));
                    }
                }
                _ => {
                    if var.value.contains(' ') || var.value.contains('$') {
                        lines.push(format!("export {}=\"{}\"", var.name, var.value));
                    } else {
                        lines.push(format!("export {}={}", var.name, var.value));
                    }
                }
            }
        }
        lines.push(String::new());
    }

    lines.push("# === End Podman TUI Managed ===".to_string());

    lines.join("\n")
}

// ─── Formatting ───────────────────────────────────────────────────────────────

pub fn format_env_var(shell: ShellKind, var: &EnvVar) -> String {
    if !var.enabled {
        return format!("# {}", format_env_var_for_shell(shell, &var.name, &var.value));
    }
    format_env_var_for_shell(shell, &var.name, &var.value)
}

fn format_env_var_for_shell(shell: ShellKind, name: &str, value: &str) -> String {
    match shell {
        ShellKind::Fish => {
            if value.contains(' ') {
                format!("set -gx {} \"{}\"", name, value)
            } else {
                format!("set -gx {} {}", name, value)
            }
        }
        _ => {
            if value.contains(' ') || value.contains('$') {
                format!("export {}=\"{}\"", name, value)
            } else {
                format!("export {}={}", name, value)
            }
        }
    }
}

pub fn format_path_entry_single(shell: ShellKind, path: &str) -> String {
    match shell {
        ShellKind::Fish => format!("fish_add_path {}", path),
        _ => format!("export PATH=\"{}:$PATH\"", path),
    }
}
