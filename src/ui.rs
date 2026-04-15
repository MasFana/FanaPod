use ratatui::{
    prelude::*,
    widgets::{Block, BorderType, Borders, Cell, Clear, Paragraph, Row, Table, Wrap},
};

use std::cmp;

use crate::app::{App, AppMode, ShellManagerTab, SortBy};
use crate::shell::{format_env_var, format_path_entry_single, shell_export_syntax, ShellKind};

fn fmt_pct(v: f64) -> String {
    format!("{:.1}%", v.max(0.0))
}

pub fn render(frame: &mut Frame, app: &mut App) {
    let show_help = matches!(app.mode, AppMode::ShellManager) && app.shell_manager.show_help;
    let bottom_height = if show_help { 18 } else { 3 };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(bottom_height),
        ])
        .split(frame.area());

    match app.mode {
        AppMode::Dashboard => render_dashboard(frame, app, &chunks),
        AppMode::ShellManager => render_shell_manager(frame, app, &chunks),
    }
}

fn render_dashboard(frame: &mut Frame, app: &mut App, chunks: &[Rect]) {
    render_title(frame, chunks[0], app);

    let w = frame.area().width;

    let body_chunks = if w >= 120 {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(chunks[1])
    } else if w >= 80 {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(chunks[1])
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(chunks[1])
    };

    render_container_list(frame, body_chunks[0], app);

    let h = body_chunks[1].height;
    let right_chunks = if h >= 14 {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(10), Constraint::Min(0)])
            .split(body_chunks[1])
    } else if h >= 10 {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(8), Constraint::Min(0)])
            .split(body_chunks[1])
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(6), Constraint::Min(0)])
            .split(body_chunks[1])
    };

    render_stats_panel(frame, right_chunks[0], app);
    render_details(frame, right_chunks[1], app);

    render_action_bar(frame, &chunks[2], app);
}

fn render_title(frame: &mut Frame, area: Rect, app: &App) {
    let sort_str = match app.sort_by {
        SortBy::Name => "Name",
        SortBy::Status => "Status",
        SortBy::Cpu => "Cpu",
        SortBy::Memory => "Memory",
    };

    let title_text = if area.width >= 50 {
        Line::from(vec![
            Span::styled(
                "Podman TUI Dashboard",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" ".repeat(area.width.saturating_sub(65) as usize)),
            Span::styled(
                format!("[sorted by: {}]", sort_str),
                Style::default().fg(Color::Cyan),
            ),
        ])
    } else if area.width >= 30 {
        Line::from(vec![
            Span::styled(
                "Podman TUI",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                format!("[{}]", sort_str),
                Style::default().fg(Color::Cyan),
            ),
        ])
    } else {
        Line::from(vec![
            Span::styled(
                "Podman TUI",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ])
    };

    let title = Paragraph::new(title_text).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Plain)
            .border_style(Style::default().fg(Color::Gray)),
    );

    frame.render_widget(title, area);
}

fn render_container_list(frame: &mut Frame, area: Rect, app: &mut App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Gray));

    if app.containers.is_empty() {
        let no_data = Paragraph::new("No containers found")
            .block(block)
            .alignment(Alignment::Center);
        frame.render_widget(no_data, area);
        return;
    }

    let max_name: usize = app
        .containers
        .iter()
        .map(|c| c.name.len() + 2)
        .max()
        .unwrap_or(10)
        .max(6);
    let max_status: usize = app
        .containers
        .iter()
        .map(|c| c.status.len())
        .max()
        .unwrap_or(10)
        .max(6);
    let inner_width = area.width.saturating_sub(2);
    let cpu_col: u16 = 8;
    let mem_min: u16 = 20;
    let needed = (max_name as u16) + (max_status as u16) + cpu_col + mem_min;

    if inner_width >= needed {
        let header = Row::new(vec!["NAME", "STATUS", "CPU%", "MEM%"])
            .style(Style::default().add_modifier(Modifier::BOLD));

        let rows: Vec<Row> = app
            .containers
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let stats = app.stats.iter().find(|s| s.name == c.name);
                let cpu = stats.map_or(String::from("--"), |s| fmt_pct(s.cpu_percent));
                let mem = stats.map_or(String::from("--"), |s| s.mem_usage.clone());

                let (indicator, color) = match c.state.to_lowercase().as_str() {
                    "running" => ("●", Color::Green),
                    "paused" | "stopped" => ("○", Color::Yellow),
                    _ => ("○", Color::Gray),
                };

                let name_cell = Line::from(vec![
                    Span::styled(indicator, Style::default().fg(color)),
                    Span::raw(" "),
                    Span::raw(c.name.clone()),
                ]);

                let row = Row::new(vec![
                    Cell::from(name_cell),
                    Cell::from(c.status.clone()),
                    Cell::from(cpu),
                    Cell::from(mem),
                ]);

                if i == app.selected {
                    row.style(Style::default().bg(Color::DarkGray))
                } else {
                    row.style(Style::default())
                }
            })
            .collect();

        let mem_col: u16 = inner_width
            .saturating_sub(max_name as u16)
            .saturating_sub(max_status as u16)
            .saturating_sub(cpu_col)
            .max(mem_min);
        let name_col: u16 = inner_width
            .saturating_sub(max_status as u16)
            .saturating_sub(cpu_col)
            .saturating_sub(mem_col)
            .max(max_name as u16);
        let status_col: u16 = inner_width
            .saturating_sub(name_col)
            .saturating_sub(cpu_col)
            .saturating_sub(mem_col)
            .max(max_status as u16);

        let widths = [
            Constraint::Length(name_col),
            Constraint::Length(status_col),
            Constraint::Length(cpu_col),
            Constraint::Fill(1),
        ];

        let table = Table::new(rows, widths).header(header).block(block);
        frame.render_widget(table, area);
    } else {
        let header = Row::new(vec!["NAME", "STATUS", "CPU"])
            .style(Style::default().add_modifier(Modifier::BOLD));

        let rows: Vec<Row> = app
            .containers
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let stats = app.stats.iter().find(|s| s.name == c.name);
                let cpu = stats.map_or(String::from("--"), |s| fmt_pct(s.cpu_percent));

                let (indicator, color) = match c.state.to_lowercase().as_str() {
                    "running" => ("●", Color::Green),
                    "paused" | "stopped" => ("○", Color::Yellow),
                    _ => ("○", Color::Gray),
                };

                let name_cell = Line::from(vec![
                    Span::styled(indicator, Style::default().fg(color)),
                    Span::raw(" "),
                    Span::raw(c.name.clone()),
                ]);

                let row = Row::new(vec![
                    Cell::from(name_cell),
                    Cell::from(c.status.clone()),
                    Cell::from(cpu),
                ]);

                if i == app.selected {
                    row.style(Style::default().bg(Color::DarkGray))
                } else {
                    row.style(Style::default())
                }
            })
            .collect();

        let avail = inner_width.saturating_sub(max_name as u16).saturating_sub(cpu_col);
        let name_col: u16 = inner_width
            .saturating_sub(avail.max(max_status as u16))
            .saturating_sub(cpu_col)
            .max(max_name as u16);
        let status_col: u16 = avail.saturating_sub(cpu_col).max(max_status as u16);

        let widths = [
            Constraint::Length(name_col),
            Constraint::Length(status_col),
            Constraint::Fill(1),
        ];

        let table = Table::new(rows, widths).header(header).block(block);
        frame.render_widget(table, area);
    }
}

fn render_stats_panel(frame: &mut Frame, area: Rect, app: &App) {
    let total = app.containers.len();
    let running = app
        .containers
        .iter()
        .filter(|c| c.state.to_lowercase() == "running")
        .count();
    let stopped = total - running;

    let total_cpu: f64 = app.stats.iter().map(|s| s.cpu_percent).sum();

    let total_net_in: f64 = app.stats.iter().map(|s| parse_io_value(&s.net_io, true)).sum();
    let total_net_out: f64 = app.stats.iter().map(|s| parse_io_value(&s.net_io, false)).sum();
    let total_blk_in: f64 = app.stats.iter().map(|s| parse_io_value(&s.block_io, true)).sum();
    let total_blk_out: f64 = app.stats.iter().map(|s| parse_io_value(&s.block_io, false)).sum();

    let total_mem_bytes: f64 = app.stats.iter().map(|s| parse_mem_usage_bytes(&s.mem_usage)).sum();

    let sys = &app.system_info;
    let sys_cpu_cores = sys.cpu_cores.max(1) as f64;
    let sys_total_mem = sys.total_mem_bytes as f64;
    let sys_used_mem = sys.used_mem_bytes as f64;

    let sys_cpu_pct = (total_cpu / (sys_cpu_cores * 100.0)) * 100.0;
    let sys_mem_pct = if sys_total_mem > 0.0 {
        (sys_used_mem / sys_total_mem) * 100.0
    } else {
        0.0
    };

    let sys_swap_pct = sys.swap.pct();
    let sys_swap_total = sys.swap.total_bytes as f64;
    let sys_swap_used = sys.swap.used_bytes as f64;

    let inner_h = area.height.saturating_sub(2);

    let mut text = vec![];

    // Container CPU/MEM + count
    if inner_h >= 1 {
        text.push(Line::from(vec![
            Span::styled("CPU ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled(fmt_pct(total_cpu), Style::default().fg(Color::Cyan)),
            Span::styled("  MEM ", Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
            Span::styled(format!("{} / {} ({:.1}%)", format_bytes(total_mem_bytes), format_bytes(sys_total_mem), sys_mem_pct), Style::default().fg(Color::Magenta)),
        ]));
    }
    if inner_h >= 2 {
        text.push(Line::from(vec![
            Span::styled("Containers: ", Style::default().fg(Color::Gray)),
            Span::styled(format!("{}/{}/{}", total, running, stopped), Style::default().fg(Color::White)),
        ]));
    }
    if inner_h >= 3 {
        text.push(Line::from(vec![
            Span::styled("NetIO  ", Style::default().fg(Color::Green)),
            Span::styled(format!("↓{}  ↑{}", format_bytes(total_net_in), format_bytes(total_net_out)), Style::default().fg(Color::Green)),
            Span::styled("  BlockIO", Style::default().fg(Color::White)),
            Span::styled(format!(" ↓{}  ↑{}", format_bytes(total_blk_in), format_bytes(total_blk_out)), Style::default().fg(Color::White)),
        ]));
    }

    // Divider
    if inner_h >= 4 {
        let w = area.width.saturating_sub(2) as usize;
        text.push(Line::from(vec![
            Span::styled("─".repeat(w), Style::default().fg(Color::DarkGray)),
        ]));
    }

    // System CPU/MEM
    if inner_h >= 5 {
        text.push(Line::from(vec![
            Span::styled("CPU ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::styled(format!("{:.1}%", sys_cpu_pct), Style::default().fg(Color::Green)),
            Span::styled("  MEM ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(format!("{} / {} ({:.1}%)", format_bytes(sys_used_mem), format_bytes(sys_total_mem), sys_mem_pct), Style::default().fg(Color::Yellow)),
        ]));
    }

    // SWAP + System Net on same line if space allows
    if inner_h >= 6 && sys_swap_total > 0.0 {
        text.push(Line::from(vec![
            Span::styled("SWAP ", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)),
            Span::styled(format!("{} / {} ({:.1}%)", format_bytes(sys_swap_used), format_bytes(sys_swap_total), sys_swap_pct), Style::default().fg(Color::Blue)),
        ]));
    }
    if inner_h >= (if sys_swap_total > 0.0 { 7 } else { 6 }) {
        text.push(Line::from(vec![
            Span::styled("Net   ", Style::default().fg(Color::Green)),
            Span::styled(format!("↓{}/s  ↑{}/s", format_bytes(app.net_rx_speed), format_bytes(app.net_tx_speed)), Style::default().fg(Color::Green)),
        ]));
    }

    let p = Paragraph::new(text).block(
        Block::default()
            .title("Resource Summary")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Gray)),
    );

    frame.render_widget(p, area);
}

fn parse_io_value(io_str: &str, inbound: bool) -> f64 {
    let parts: Vec<&str> = io_str.split('/').collect();
    let target = if inbound { parts.first() } else { parts.get(1) };
    target.map(|s| parse_byte_string(s.trim())).unwrap_or(0.0)
}

fn parse_byte_string(s: &str) -> f64 {
    let s = s.trim();
    if s.is_empty() || s == "--" || s == "0B" {
        return 0.0;
    }

    let num_str: String = s.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
    let num: f64 = num_str.parse().unwrap_or(0.0);
    let unit: String = s.chars().skip(num_str.len()).collect();

    match unit.to_uppercase().as_str() {
        "B" => num,
        "KB" | "KIB" => num * 1_024.0,
        "MB" | "MIB" => num * 1_048_576.0,
        "GB" | "GIB" => num * 1_073_741_824.0,
        "TB" | "TIB" => num * 1_099_511_627_776.0,
        _ => num,
    }
}

fn parse_mem_usage_bytes(mem_str: &str) -> f64 {
    let parts: Vec<&str> = mem_str.split('/').collect();
    parts.first().map(|s| parse_byte_string(s.trim())).unwrap_or(0.0)
}

fn format_bytes(bytes: f64) -> String {
    if bytes >= 1_099_511_627_776.0 {
        format!("{:.2}TB", bytes / 1_099_511_627_776.0)
    } else if bytes >= 1_073_741_824.0 {
        format!("{:.2}GB", bytes / 1_073_741_824.0)
    } else if bytes >= 1_048_576.0 {
        format!("{:.1}MB", bytes / 1_048_576.0)
    } else if bytes >= 1_024.0 {
        format!("{:.1}KB", bytes / 1_024.0)
    } else {
        format!("{:.0}B", bytes)
    }
}

fn render_details(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title("Container Details")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Gray));

    if let Some(c) = app.containers.get(app.selected) {
        let short_id = if c.id.len() > 12 {
            &c.id[0..12]
        } else {
            &c.id
        };

        let max_name_len = (area.width.saturating_sub(8)) as usize;
        let truncated_name = if c.name.len() > max_name_len {
            format!("{}…", &c.name[..max_name_len.saturating_sub(1)])
        } else {
            c.name.clone()
        };
        let truncated_image = if c.image.len() > max_name_len {
            format!("{}…", &c.image[..max_name_len.saturating_sub(1)])
        } else {
            c.image.clone()
        };

        let mut text = vec![
            Line::from(format!("Name: {}", truncated_name)),
            Line::from(format!("ID: {}", short_id)),
            Line::from(format!("Image: {}", truncated_image)),
            Line::from(format!("Status: {}", c.status)),
        ];

        if let Some(s) = app.stats.iter().find(|s| s.name == c.name) {
            text.push(Line::from(""));
            text.push(Line::from(format!("CPU: {}", fmt_pct(s.cpu_percent))));
            text.push(Line::from(format!("Mem: {} ({})", fmt_pct(s.mem_percent), s.mem_usage)));
            text.push(Line::from(format!("NetIO: {}", s.net_io)));
            text.push(Line::from(format!("BlockIO: {}", s.block_io)));
        }

        let p = Paragraph::new(text).block(block).wrap(Wrap { trim: true });
        frame.render_widget(p, area);
    } else {
        let p = Paragraph::new("No container selected")
            .block(block)
            .alignment(Alignment::Center);
        frame.render_widget(p, area);
    }
}

fn render_shell_manager(frame: &mut Frame, app: &mut App, chunks: &[Rect]) {
    let w = frame.area().width;
    let is_editing = app.shell_manager.editing.is_active();

    if app.shell_manager.show_help {
        render_help_overlay(frame, chunks[1]);
        render_shell_action_bar(frame, &chunks[2], app);
        return;
    }

    if w >= 110 {
        let top_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(50), Constraint::Min(0)])
            .split(chunks[0]);
        render_tab_bar(frame, top_chunks[0], app);
        render_shell_selector(frame, top_chunks[1], app);
    } else if w >= 70 {
        let top_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(38), Constraint::Min(0)])
            .split(chunks[0]);
        render_tab_bar(frame, top_chunks[0], app);
        render_shell_selector(frame, top_chunks[1], app);
    } else {
        render_tab_bar(frame, chunks[0], app);
    }

    let body_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(chunks[1]);

    match app.shell_manager.active_tab {
        ShellManagerTab::Variables => {
            render_env_vars_panel(frame, body_chunks[0], app);
        }
        ShellManagerTab::Paths => {
            render_path_entries_panel(frame, body_chunks[0], app);
        }
    }
    render_shell_config_preview(frame, body_chunks[1], app);

    if is_editing {
        render_edit_tooltip(frame, chunks[2], app);
    } else {
        render_shell_action_bar(frame, &chunks[2], app);
    }

    if is_editing {
        render_modal_popup(frame, frame.area(), app);
    }
}

fn render_help_overlay(frame: &mut Frame, area: Rect) {
    let block = Block::default()
        .title("Keyboard Shortcuts")
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(Color::Cyan));

    let bindings = App::keybindings_help();
    let mut lines: Vec<Line> = vec![];

    for (key, desc) in bindings {
        if key.is_empty() && desc.is_empty() {
            lines.push(Line::from(""));
        } else if desc.is_empty() {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {}", key),
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                ),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("    {:<14}", key),
                    Style::default().fg(Color::Yellow),
                ),
                Span::styled(desc, Style::default().fg(Color::Gray)),
            ]));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled(
            "  Press Esc, ?, or q to close",
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM),
        ),
    ]));

    let p = Paragraph::new(lines).block(block).wrap(Wrap { trim: false });
    frame.render_widget(p, area);
}

fn render_tab_bar(frame: &mut Frame, area: Rect, app: &App) {
    let is_dashboard = matches!(app.mode, AppMode::Dashboard);
    let is_shell = matches!(app.mode, AppMode::ShellManager);
    let is_vars = matches!(app.shell_manager.active_tab, ShellManagerTab::Variables);
    let is_paths = matches!(app.shell_manager.active_tab, ShellManagerTab::Paths);

    let dashboard_style = if is_dashboard { Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::Gray) };
    let shell_style = if is_shell { Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::Gray) };
    let vars_style = if is_vars && is_shell { Style::default().fg(Color::Green).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::Gray) };
    let paths_style = if is_paths && is_shell { Style::default().fg(Color::Green).add_modifier(Modifier::BOLD) } else { Style::default().fg(Color::Gray) };

    let w = area.width;
    let tabs = if w >= 36 {
        Line::from(vec![
            Span::styled(" Dashboard ", dashboard_style),
            Span::styled(" | ", Style::default().fg(Color::Gray)),
            Span::styled(" Shell ", shell_style),
            Span::styled(" [", Style::default().fg(Color::DarkGray)),
            Span::styled("1:Vars", vars_style),
            Span::styled(" | ", Style::default().fg(Color::DarkGray)),
            Span::styled("2:Paths", paths_style),
            Span::styled("]", Style::default().fg(Color::DarkGray)),
        ])
    } else if w >= 26 {
        Line::from(vec![
            Span::styled(" Dash ", dashboard_style),
            Span::styled(" | ", Style::default().fg(Color::Gray)),
            Span::styled(" Shell ", shell_style),
            Span::styled(" [", Style::default().fg(Color::DarkGray)),
            Span::styled("V", vars_style),
            Span::styled("/", Style::default().fg(Color::DarkGray)),
            Span::styled("P", paths_style),
            Span::styled("]", Style::default().fg(Color::DarkGray)),
        ])
    } else {
        Line::from(vec![
            Span::styled("Shell", shell_style),
            Span::styled(" [", Style::default().fg(Color::DarkGray)),
            Span::styled("V", vars_style),
            Span::styled("/", Style::default().fg(Color::DarkGray)),
            Span::styled("P", paths_style),
            Span::styled("]", Style::default().fg(Color::DarkGray)),
        ])
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(Color::Gray));

    let paragraph = Paragraph::new(tabs).block(block);
    frame.render_widget(paragraph, area);
}

fn render_shell_selector(frame: &mut Frame, area: Rect, app: &App) {
    let available = &app.shell_manager.available_shells;
    let selected = &app.shell_manager.selected_shell;
    let config_path = &app.shell_manager.config_path;

    let shell_display: Vec<String> = available.iter().map(|s| {
        if s == selected {
            format!("[{}]", s.to_string().to_uppercase())
        } else {
            s.to_string()
        }
    }).collect();

    let host_shell = crate::shell::detect_host_shell();
    let detected_label = if *selected == host_shell {
        " (detected)"
    } else {
        ""
    };

    let text = Line::from(vec![
        Span::styled("Shell: ", Style::default().fg(Color::Gray)),
        Span::styled(shell_display.join("  "), Style::default().fg(Color::Cyan)),
        Span::styled(detected_label, Style::default().fg(Color::Green)),
        Span::raw("  "),
        Span::styled("<,> ", Style::default().fg(Color::Yellow)),
        Span::styled("switch", Style::default().fg(Color::Gray)),
        Span::raw("  "),
        Span::styled(format!("~{}", config_path), Style::default().fg(Color::Yellow).add_modifier(Modifier::DIM)),
    ]);

    let block = Block::default()
        .title("Active Shell")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Gray));

    let paragraph = Paragraph::new(text).block(block);
    frame.render_widget(paragraph, area);
}

fn render_env_vars_panel(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title("Environment Variables")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Gray));

    if app.shell_manager.env_vars.is_empty() {
        let msg = Paragraph::new("No variables configured - press 'a' to add")
            .block(block)
            .alignment(Alignment::Center);
        frame.render_widget(msg, area);
        return;
    }

    let max_name: usize = app
        .shell_manager
        .env_vars
        .iter()
        .map(|v| v.name.len() + 2)
        .max()
        .unwrap_or(8)
        .max(8);
    let max_value: usize = app
        .shell_manager
        .env_vars
        .iter()
        .map(|v| v.value.len() + 2)
        .max()
        .unwrap_or(10)
        .max(10);

    let inner_width = area.width.saturating_sub(2);
    let enabled_col: u16 = 9;

    let name_col: u16 = inner_width
        .saturating_sub(max_value as u16)
        .saturating_sub(enabled_col)
        .max(max_name as u16)
        .min(inner_width.saturating_sub(20));
    let value_col: u16 = inner_width
        .saturating_sub(name_col)
        .saturating_sub(enabled_col)
        .max(max_value as u16);

    let header = Row::new(vec!["NAME", "VALUE", "ENABLED"])
        .style(Style::default().add_modifier(Modifier::BOLD));

    let rows: Vec<Row> = app
        .shell_manager
        .env_vars
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let enabled_symbol = if v.enabled {
                Span::styled("✓", Style::default().fg(Color::Green))
            } else {
                Span::styled("✗", Style::default().fg(Color::Gray))
            };

            let row = Row::new(vec![
                Cell::from(v.name.clone()),
                Cell::from(v.value.clone()),
                Cell::from(Line::from(enabled_symbol)),
            ]);

            if i == app.shell_manager.selected {
                row.style(Style::default().bg(Color::DarkGray))
            } else {
                row.style(Style::default())
            }
        })
        .collect();

    let widths = [
        Constraint::Length(name_col),
        Constraint::Length(value_col),
        Constraint::Fill(1),
    ];

    let table = Table::new(rows, widths).header(header).block(block);
    frame.render_widget(table, area);
}

fn render_path_entries_panel(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title("PATH Entries")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Gray));

    if app.shell_manager.path_entries.is_empty() {
        let msg = Paragraph::new("No PATH entries loaded")
            .block(block)
            .alignment(Alignment::Center);
        frame.render_widget(msg, area);
        return;
    }

    let max_path: usize = app
        .shell_manager
        .path_entries
        .iter()
        .map(|p| p.path.len() + 2)
        .max()
        .unwrap_or(20)
        .max(20);
    let max_label: usize = app
        .shell_manager
        .path_entries
        .iter()
        .filter_map(|p| p.label.as_ref().map(|l| l.len() + 2))
        .max()
        .unwrap_or(8)
        .max(8);

    let inner_width = area.width.saturating_sub(2);
    let enabled_col: u16 = 9;

    let path_col: u16 = inner_width
        .saturating_sub(max_label as u16)
        .saturating_sub(enabled_col)
        .max(max_path as u16)
        .min(inner_width.saturating_sub(20));
    let label_col: u16 = inner_width
        .saturating_sub(path_col)
        .saturating_sub(enabled_col)
        .max(max_label as u16);

    let header = Row::new(vec!["PATH", "LABEL", "ENABLED"])
        .style(Style::default().add_modifier(Modifier::BOLD));

    let rows: Vec<Row> = app
        .shell_manager
        .path_entries
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let enabled_symbol = if p.enabled {
                Span::styled("✓", Style::default().fg(Color::Green))
            } else {
                Span::styled("✗", Style::default().fg(Color::Gray))
            };

            let label = p.label.as_deref().unwrap_or("-");

            let row = Row::new(vec![
                Cell::from(p.path.clone()),
                Cell::from(label.to_string()),
                Cell::from(Line::from(enabled_symbol)),
            ]);

            if i == app.shell_manager.selected {
                row.style(Style::default().bg(Color::DarkGray))
            } else {
                row.style(Style::default())
            }
        })
        .collect();

    let widths = [
        Constraint::Length(path_col),
        Constraint::Length(label_col),
        Constraint::Fill(1),
    ];

    let table = Table::new(rows, widths).header(header).block(block);
    frame.render_widget(table, area);
}

fn render_shell_config_preview(frame: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title("Config Preview")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Gray));

    let selected_shell = &app.shell_manager.selected_shell;
    let config_path = &app.shell_manager.config_path;
    let (export_prefix, _) = shell_export_syntax(*selected_shell);

    let mut text = vec![
        Line::from(vec![
            Span::styled("File: ", Style::default().fg(Color::Gray)),
            Span::styled(format!("~{}", config_path), Style::default().fg(Color::Yellow)),
        ]),
        Line::from(vec![
            Span::styled("Syntax: ", Style::default().fg(Color::Gray)),
            Span::styled(export_prefix, Style::default().fg(Color::Cyan)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("# Environment Variables", Style::default().fg(Color::Gray).add_modifier(Modifier::DIM)),
        ]),
    ];

    let enabled_vars: Vec<_> = app
        .shell_manager
        .env_vars
        .iter()
        .filter(|v| v.enabled)
        .collect();

    for var in &enabled_vars {
        text.push(Line::from(format_env_var(*selected_shell, var)));
    }

    if enabled_vars.is_empty() {
        text.push(Line::from(Span::styled("# No enabled variables", Style::default().fg(Color::DarkGray))));
    }

    text.push(Line::from(""));
    text.push(Line::from(vec![
        Span::styled("# PATH", Style::default().fg(Color::Gray).add_modifier(Modifier::DIM)),
    ]));

    let enabled_paths: Vec<_> = app
        .shell_manager
        .path_entries
        .iter()
        .filter(|p| p.enabled)
        .collect();

    if !enabled_paths.is_empty() {
        match selected_shell {
            ShellKind::Fish => {
                for entry in &enabled_paths {
                    text.push(Line::from(format_path_entry_single(*selected_shell, &entry.path)));
                }
            }
            _ => {
                let paths: Vec<&str> = enabled_paths.iter().map(|p| p.path.as_str()).collect();
                let path_value = paths.join(":");
                text.push(Line::from(format!("export PATH=\"{}\"", path_value)));
            }
        }
    } else {
        text.push(Line::from(Span::styled("# No enabled paths", Style::default().fg(Color::DarkGray))));
    }

    let p = Paragraph::new(text).block(block).wrap(Wrap { trim: true });
    frame.render_widget(p, area);
}

fn render_modal_popup(frame: &mut Frame, area: Rect, app: &App) {
    let Some((title, label, buffer, hint)) = app.modal_display() else { return };

    let popup_width = cmp::min(70u16, area.width.saturating_sub(4));
    let popup_height = 7u16;
    let popup_x = area.width.saturating_sub(popup_width) / 2;
    let popup_y = area.height.saturating_sub(popup_height) / 2;

    let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

    let clear = Clear;
    frame.render_widget(clear, popup_area);

    let border_style = Style::default().fg(Color::Cyan);
    let block = Block::default()
        .title(format!(" {} ", title))
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(border_style);

    let inner = block.inner(popup_area);
    frame.render_widget(block, popup_area);

    let cursor_char = if buffer.is_empty() { "▌".to_string() } else { format!("{}▌", buffer) };

    let label_line = Line::from(vec![
        Span::styled(format!("{}: ", label), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::styled(cursor_char, Style::default().fg(Color::White)),
    ]);
    let hint_line = Line::from(vec![
        Span::styled(hint, Style::default().fg(Color::DarkGray)),
    ]);
    let actions_line = Line::from(vec![
        Span::styled("Enter", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        Span::styled(" confirm  ", Style::default().fg(Color::Gray)),
        Span::styled("Esc", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
        Span::styled(" cancel  ", Style::default().fg(Color::Gray)),
        Span::styled("Backspace", Style::default().fg(Color::White)),
        Span::styled(" delete", Style::default().fg(Color::Gray)),
    ]);

    let inner_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);

    frame.render_widget(Paragraph::new(label_line), inner_chunks[0]);
    frame.render_widget(Paragraph::new(hint_line), inner_chunks[1]);
    frame.render_widget(Paragraph::new(actions_line), inner_chunks[2]);
}

fn render_edit_tooltip(frame: &mut Frame, area: Rect, app: &App) {
    let Some((title, label, buffer, hint)) = app.modal_display() else { return };

    let block = Block::default()
        .title(format!(" {} ", title))
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(Color::Yellow));

    let cursor_char = if buffer.is_empty() { "▌".to_string() } else { format!("{}▌", buffer) };

    let label_line = Line::from(vec![
        Span::styled(format!("{}: ", label), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::styled(cursor_char, Style::default().fg(Color::White)),
    ]);
    let hint_line = Line::from(vec![
        Span::styled(hint, Style::default().fg(Color::DarkGray)),
    ]);
    let actions_line = Line::from(vec![
        Span::styled("Enter", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        Span::styled(" confirm  ", Style::default().fg(Color::Gray)),
        Span::styled("Esc", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
        Span::styled(" cancel  ", Style::default().fg(Color::Gray)),
        Span::styled("Backspace", Style::default().fg(Color::White)),
        Span::styled(" delete", Style::default().fg(Color::Gray)),
    ]);

    let inner_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(block.inner(area));

    frame.render_widget(block, area);
    frame.render_widget(Paragraph::new(label_line), inner_chunks[0]);
    frame.render_widget(Paragraph::new(hint_line), inner_chunks[1]);
    frame.render_widget(Paragraph::new(actions_line), inner_chunks[2]);
}

fn render_shell_action_bar(frame: &mut Frame, area: &Rect, app: &App) {
    let tab_label = match app.shell_manager.active_tab {
        ShellManagerTab::Variables => "Vars",
        ShellManagerTab::Paths => "Paths",
    };

    let is_editing = app.shell_manager.editing.is_active();
    let shell_names: Vec<String> = app.shell_manager.available_shells.iter().map(|s| s.to_string()).collect();
    let shell_hint = if shell_names.is_empty() {
        String::new()
    } else {
        format!(" [{}]", shell_names.join("/"))
    };

    let bindings = if area.width >= 100 {
        format!("[j/k]Nav [a]Add [d]Del [e]Edit [r]Rename [t]Toggle [s]Save [Tab]Tab [1/2]Jump [<,>]Shell{} [?]Help [v]Back [q]Quit", shell_hint)
    } else if area.width >= 70 {
        format!("[j/k]Nav [a]Add [d]Del [e]Edit [t]Toggle [s]Save [?]Help [q]Quit")
    } else {
        "j/k Nav | a Add | d Del | e Edit | s Save | q Quit".to_string()
    };

    let mut spans = vec![Span::styled(
        bindings,
        Style::default().fg(Color::White).add_modifier(Modifier::DIM),
    )];

    if is_editing {
        spans.push(Span::raw(" | "));
        spans.push(Span::styled(
            "Type input... Enter=confirm Esc=cancel",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ));
    } else if let Some(msg) = &app.shell_manager.message {
        spans.push(Span::raw(" | "));
        spans.push(Span::styled(msg.clone(), Style::default().fg(Color::Green)));
    } else if let Some(err) = &app.shell_manager.error {
        spans.push(Span::raw(" | "));
        spans.push(Span::styled(err.clone(), Style::default().fg(Color::Red)));
    }

    spans.push(Span::raw(" | "));
    spans.push(Span::styled(
        format!("Active:{}", tab_label),
        Style::default().fg(Color::Cyan),
    ));

    let p = Paragraph::new(Line::from(spans)).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Gray)),
    );

    frame.render_widget(p, *area);
}

fn render_action_bar(frame: &mut Frame, area: &Rect, app: &App) {
    let bindings = if area.width >= 100 {
        " [j/k]Nav [s]Start [x]Stop [d]Delete [e]Exec [E]Root [r]Refresh [t]Sort [b]Terminal [v/m]ShellMgr [?]Help [q]Quit "
    } else if area.width >= 70 {
        " [j/k]Nav [s]Start [x]Stop [d]Del [e]Exec [E]Root [b]Term [v]ShellMgr [q]Quit "
    } else if area.width >= 50 {
        " [j/k]Nav [s]Start [x]Stop [e]Exec [b]Term [q]Quit "
    } else {
        " j/k Nav | s Start | x Stop | e Exec | q Quit "
    };
    
    let mut spans = vec![Span::styled(
        bindings,
        Style::default().fg(Color::White).add_modifier(Modifier::DIM),
    )];

    if let Some(action) = &app.pending_action {
        spans.push(Span::raw(" | "));
        let (label, name) = match action {
            crate::app::PendingAction::Stop(n) => ("Stop", n),
            crate::app::PendingAction::Delete(n) => ("Delete", n),
        };
        spans.push(Span::styled(
            format!("[y] Confirm {} '{}'", label, name),
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            "[Esc] Cancel",
            Style::default().fg(Color::Gray),
        ));
    } else if let Some(err) = &app.error_message {
        spans.push(Span::raw(" | "));
        spans.push(Span::styled(err.clone(), Style::default().fg(Color::Red)));
    } else if let Some(msg) = &app.status_message {
        spans.push(Span::raw(" | "));
        spans.push(Span::styled(msg.clone(), Style::default().fg(Color::Green)));
    }

    let p = Paragraph::new(Line::from(spans)).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Gray)),
    );

    frame.render_widget(p, *area);
}
