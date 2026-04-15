# podman-tui

A terminal-based user interface (TUI) for Podman, built with Rust and Ratatui. It provides a dashboard for monitoring container stats and a shell manager for configuring your host environment.

## Features

- **Container Dashboard**:
    - Monitor running/stopped containers.
    - Real-time CPU and Memory usage statistics.
    - Network I/O and Block I/O monitoring.
    - System-wide resource overview (CPU, RAM, Swap).
- **Shell Manager**:
    - Manage environment variables and PATH entries.
    - Supports Bash, Zsh, and Fish shells.
    - Easily toggle, add, or edit configuration directly from the TUI.
- **Interactive Controls**:
    - Sorting by name, status, CPU, or memory.
    - Fast and responsive interface.

## Preview

### Container Dashboard
```text
┌────────────────────────────────────────────────────────────────────────────┐
│ Podman TUI Dashboard                                  [sorted by: Name]    │
├────────────────────────────────────────────────────────────────────────────┤
│ NAME            STATUS          CPU%    MEM%      ┌────────────────────────┐
│ ● postgres      Up 2 hours      0.5%    124.2MB   │ Resource Summary       │
│ ● redis         Up 2 hours      0.1%    12.5MB    │ CPU 0.6%  MEM 136.7MB  │
│ ○ nginx         Exited (0)      --      --        │ Containers: 3/2/1      │
│                                                   │ NetIO ↓1.2KB ↑0.5KB    │
│                                                   │ ────────────────────── │
│                                                   │ CPU 12.4% MEM 4.2GB    │
│                                                   │ Net ↓45.2KB/s ↑12.1KB/s│
│                                                   └────────────────────────┘
│                                                   ┌────────────────────────┐
│                                                   │ Container Details      │
│                                                   │ Name: postgres         │
│                                                   │ ID: a1b2c3d4e5f6       │
│                                                   │ Image: postgres:15     │
│                                                   │ Status: Up 2 hours     │
│                                                   └────────────────────────┘
├────────────────────────────────────────────────────────────────────────────┤
│ [j/k]Nav [s]Start [x]Stop [d]Delete [e]Exec [E]Root [r]Refresh [q]Quit     │
└────────────────────────────────────────────────────────────────────────────┘
```

### Shell Manager (Variables)
```text
┌────────────────────────────────────────────────────────────────────────────┐
│  Dashboard  |  Shell  [1:Vars | 2:Paths]        Shell: [BASH]  zsh  fish   │
├──────────────────────────────────────────────────┬─────────────────────────┤
│ NAME            VALUE                  ENABLED   │ Config Preview          │
│ DATABASE_URL    postgres://localhost   ✓         │ File: ~/.bashrc         │
│ API_KEY         ********************   ✓         │ Syntax: export          │
│ DEBUG           true                   ✗         │                         │
│                                                  │ # Environment Variables │
│                                                  │ export DATABASE_URL=""  │
│                                                  │ export API_KEY="..."    │
│                                                  │                         │
│                                                  │ # PATH                  │
│                                                  │ export PATH="/usr/bin"  │
└──────────────────────────────────────────────────┴─────────────────────────┘
│ [j/k]Nav [a]Add [d]Del [e]Edit [t]Toggle [s]Save [Tab]Tab [?]Help [q]Quit  │
└────────────────────────────────────────────────────────────────────────────┘
```

### Shell Manager (Paths)
```text
┌────────────────────────────────────────────────────────────────────────────┐
│  Dashboard  |  Shell  [1:Vars | 2:Paths]        Shell: [BASH]  zsh  fish   │
├──────────────────────────────────────────────────┬─────────────────────────┤
│ PATH                         LABEL     ENABLED   │ Config Preview          │
│ /usr/local/bin               -         ✓         │ File: ~/.bashrc         │
│ ~/.cargo/bin                 rust      ✓         │ Syntax: export          │
│ /opt/podman/bin              podman    ✗         │                         │
│                                                  │ # Environment Variables │
│                                                  │ export DATABASE_URL=".."│
│                                                  │                         │
│                                                  │ # PATH                  │
│                                                  │ export PATH="/usr/bin:."│
└──────────────────────────────────────────────────┴─────────────────────────┘
│ [j/k]Nav [a]Add [d]Del [e]Edit [t]Toggle [s]Save [Tab]Tab [?]Help [q]Quit  │
└────────────────────────────────────────────────────────────────────────────┘
```

## Prerequisites

- **Podman**: Ensure Podman is installed and the `podman` command is available in your PATH.
- **Rust**: To build from source, you'll need the Rust toolchain (Cargo).

## Installation

```bash
# Clone the repository
git clone https://github.com/yourusername/podman-tui.git
cd podman-tui

# Build the project
cargo build --release

# Run the application
./target/release/podman-tui
```

## Usage

### Keybindings

- **Tab**: Switch between Dashboard and Shell Manager modes.
- **j/k** or **Up/Down**: Navigate through lists.
- **s**: Cycle sorting order in Dashboard.
- **r**: Refresh container list/stats.
- **q**: Quit the application.

*Specific keybindings for the Shell Manager and editing modes are available in the on-screen help.*

## Project Structure

- `src/main.rs`: Application entry point and terminal setup.
- `src/app.rs`: Core application logic and state management.
- `src/ui.rs`: UI rendering logic using Ratatui.
- `src/podman.rs`: Wrapper for Podman CLI commands and system info gathering.
- `src/shell.rs`: Logic for shell configuration parsing and management.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details (or specify your preferred license).
