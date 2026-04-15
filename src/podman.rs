use serde::Deserialize;
use std::fs;
use std::process::Command;

#[derive(Debug, Clone, Default)]
pub struct SwapInfo {
    pub total_bytes: u64,
    pub used_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct SystemInfo {
    pub cpu_cores: usize,
    pub total_mem_bytes: u64,
    pub used_mem_bytes: u64,
    pub swap: SwapInfo,
    pub net_rx_bytes: u64,
    pub net_tx_bytes: u64,
}

impl Default for SystemInfo {
    fn default() -> Self {
        Self {
            cpu_cores: 0,
            total_mem_bytes: 0,
            used_mem_bytes: 0,
            swap: SwapInfo {
                total_bytes: 0,
                used_bytes: 0,
            },
            net_rx_bytes: 0,
            net_tx_bytes: 0,
        }
    }
}

impl SwapInfo {
    pub fn pct(&self) -> f64 {
        if self.total_bytes == 0 {
            return 0.0;
        }
        (self.used_bytes as f64 / self.total_bytes as f64) * 100.0
    }
}

#[derive(Debug, Clone)]
pub struct ContainerInfo {
    pub id: String,
    pub name: String,
    pub image: String,
    pub status: String,
    pub state: String,
}

#[derive(Debug, Clone)]
pub struct ContainerStats {
    pub name: String,
    pub cpu_percent: f64,
    pub mem_usage: String,
    pub mem_percent: f64,
    pub net_io: String,
    pub block_io: String,
}

#[derive(Deserialize)]
struct RawContainerInfo {
    #[serde(rename = "Id")]
    id: Option<String>,
    #[serde(rename = "Names")]
    names: Option<NamesField>,
    #[serde(rename = "Image")]
    image: Option<String>,
    #[serde(rename = "Status")]
    status: Option<String>,
    #[serde(rename = "State")]
    state: Option<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum NamesField {
    One(String),
    Many(Vec<String>),
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct RawContainerStats {
    name: Option<String>,
    #[serde(alias = "CPUPerc")]
    cpu_percent: Option<String>,
    #[serde(alias = "MemUsage")]
    mem_usage: Option<String>,
    #[serde(alias = "MemPerc")]
    mem_percent: Option<String>,
    #[serde(alias = "NetIO")]
    net_io: Option<String>,
    #[serde(alias = "BlockIO")]
    block_io: Option<String>,
}

fn parse_percent(value: Option<&str>) -> f64 {
    value
        .unwrap_or_default()
        .trim()
        .trim_end_matches('%')
        .parse::<f64>()
        .unwrap_or(0.0)
}

fn extract_name(names: Option<NamesField>) -> String {
    match names {
        Some(NamesField::One(name)) => name,
        Some(NamesField::Many(mut names)) => names.drain(..).next().unwrap_or_default(),
        None => String::new(),
    }
}

pub fn get_system_info() -> SystemInfo {
    let cpu_cores = fs::read_to_string("/proc/cpuinfo")
        .map(|c| c.lines().filter(|l| l.starts_with("processor")).count())
        .unwrap_or(1);

    let mut total_mem_bytes = 0u64;
    let mut avail_mem_bytes = 0u64;
    let mut swap_total_kb = 0u64;
    let mut swap_free_kb = 0u64;

    if let Ok(content) = fs::read_to_string("/proc/meminfo") {
        for line in content.lines() {
            if let Some(rest) = line.strip_prefix("MemTotal:") {
                let trimmed = rest.trim();
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if parts.len() >= 2 {
                    if let Ok(kb) = parts[0].parse::<u64>() {
                        total_mem_bytes = kb * 1024;
                    }
                }
            } else if let Some(rest) = line.strip_prefix("MemAvailable:") {
                let trimmed = rest.trim();
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if parts.len() >= 2 {
                    if let Ok(kb) = parts[0].parse::<u64>() {
                        avail_mem_bytes = kb * 1024;
                    }
                }
            } else if let Some(rest) = line.strip_prefix("SwapTotal:") {
                let trimmed = rest.trim();
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if parts.len() >= 2 {
                    swap_total_kb = parts[0].parse().unwrap_or(0);
                }
            } else if let Some(rest) = line.strip_prefix("SwapFree:") {
                let trimmed = rest.trim();
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if parts.len() >= 2 {
                    swap_free_kb = parts[0].parse().unwrap_or(0);
                }
            }
        }
    }

    let used_mem_bytes = total_mem_bytes.saturating_sub(avail_mem_bytes);
    let swap = SwapInfo {
        total_bytes: swap_total_kb * 1024,
        used_bytes: swap_total_kb.saturating_sub(swap_free_kb) * 1024,
    };

    let (net_rx, net_tx) = read_network_stats();

    SystemInfo {
        cpu_cores,
        total_mem_bytes,
        used_mem_bytes,
        swap,
        net_rx_bytes: net_rx,
        net_tx_bytes: net_tx,
    }
}

fn read_network_stats() -> (u64, u64) {
    let Ok(content) = fs::read_to_string("/proc/net/dev") else {
        return (0, 0);
    };
    let mut rx = 0u64;
    let mut tx = 0u64;
    for line in content.lines().skip(2) {
        let line = line.trim();
        if line.starts_with("lo:") || line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split(':').collect();
        if parts.len() != 2 {
            continue;
        }
        let stats: Vec<&str> = parts[1].split_whitespace().collect();
        if stats.len() >= 16 {
            rx += stats[0].parse::<u64>().unwrap_or(0);
            tx += stats[8].parse::<u64>().unwrap_or(0);
        }
    }
    (rx, tx)
}

pub fn list_containers() -> Result<Vec<ContainerInfo>, String> {
    let output = Command::new("podman")
        .args(["ps", "-a", "--format", "json"])
        .output()
        .map_err(|e| format!("podman CLI not found: {}", e))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    let raw: Vec<RawContainerInfo> = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("failed to parse podman output: {}", e))?;

    Ok(raw
        .into_iter()
        .map(|container| ContainerInfo {
            id: container.id.unwrap_or_default(),
            name: extract_name(container.names),
            image: container.image.unwrap_or_default(),
            status: container.status.unwrap_or_default(),
            state: container.state.unwrap_or_default(),
        })
        .collect())
}

pub fn get_stats() -> Result<Vec<ContainerStats>, String> {
    let output = Command::new("podman")
        .args(["stats", "--no-stream", "--format", "json"])
        .output()
        .map_err(|e| format!("podman CLI not found: {}", e))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    let raw: Vec<RawContainerStats> = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("failed to parse podman output: {}", e))?;

    Ok(raw
        .into_iter()
        .map(|stats| ContainerStats {
            name: stats.name.unwrap_or_default(),
            cpu_percent: parse_percent(stats.cpu_percent.as_deref()),
            mem_usage: stats.mem_usage.unwrap_or_default(),
            mem_percent: parse_percent(stats.mem_percent.as_deref()),
            net_io: stats.net_io.unwrap_or_default(),
            block_io: stats.block_io.unwrap_or_default(),
        })
        .collect())
}

pub fn start_container(name: &str) -> Result<(), String> {
    let output = Command::new("podman")
        .args(["start", name])
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

pub fn stop_container(name: &str) -> Result<(), String> {
    let output = Command::new("podman")
        .args(["stop", name])
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

pub fn rm_container(name: &str) -> Result<(), String> {
    let output = Command::new("podman")
        .args(["rm", "-f", name])
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

fn find_terminal() -> Result<(String, Vec<String>), String> {
    if let Some((term, args)) = crate::shell::detect_host_terminal() {
        return Ok((term, args));
    }

    let candidates: [(&str, &[&str]); 5] = [
        ("ghostty", &["-e"]),
        ("alacritty", &["-e"]),
        ("kitty", &[]),
        ("konsole", &["-e"]),
        ("xterm", &["-e"]),
    ];

    for (term, args) in candidates {
        if Command::new(term).arg("--version").output().is_ok()
            || Command::new(term).arg("-h").output().is_ok()
        {
            return Ok((
                term.to_string(),
                args.iter().map(|s| s.to_string()).collect(),
            ));
        }
    }

    Err("No supported terminal emulator found".to_string())
}

pub fn exec_container(name: &str, state: &str, as_root: bool) -> Result<String, String> {
    if state.to_lowercase() != "running" {
        start_container(name)?;
    }

    let (term, term_args) = find_terminal()?;
    let user = if as_root { "root" } else { "1000" };

    let mut podman_args = vec![
        "exec".to_string(),
        "-it".to_string(),
        "-u".to_string(),
        user.to_string(),
        "-w".to_string(),
        "/workspace".to_string(),
        name.to_string(),
    ];

    if as_root {
        let shell_candidates = ["/usr/bin/fish", "/bin/zsh", "/bin/bash", "/usr/bin/zsh"];
        let detected_shell = shell_candidates
            .iter()
            .find(|&&shell_path| {
                Command::new("podman")
                    .args(["exec", name, "test", "-f", shell_path])
                    .output()
                    .map(|o| o.status.success())
                    .unwrap_or(false)
            })
            .copied()
            .unwrap_or("/bin/bash");
        podman_args.push(detected_shell.to_string());
        podman_args.push("-l".to_string());
    } else {
        // Match dev.sh behavior for non-root users
        podman_args.push("/bin/bash".to_string());
        podman_args.push("-l".to_string());
    }

    let mut cmd = Command::new(&term);
    for a in &term_args {
        cmd.arg(a);
    }

    // Always add podman as the base command
    cmd.arg("podman");
    // Add all podman arguments individually
    for arg in podman_args {
        cmd.arg(arg);
    }

    cmd.stderr(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("failed to spawn {}: {}", term, e))?;

    Ok(if state.to_lowercase() != "running" {
        format!("Started and opened: {}", name)
    } else {
        format!("Opened: {}", name)
    })
}

pub fn binary_dir() -> std::path::PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or(std::env::current_dir().unwrap_or_default())
}

pub fn open_terminal_in_bin_dir() -> Result<(), String> {
    let bin_dir = binary_dir();

    if !bin_dir.exists() {
        return Err(format!("binary directory not found: {}", bin_dir.display()));
    }

    let (term, term_args) = find_terminal()?;

    let mut cmd = Command::new(&term);
    for a in &term_args {
        // Only keep relevant flags for opening a terminal, skip execution-only flags
        if a != "-e" && a != "--" {
            cmd.arg(a.as_str());
        }
    }
    cmd.current_dir(&bin_dir);
    cmd.stderr(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("failed to spawn {}: {}", term, e))?;

    Ok(())
}
