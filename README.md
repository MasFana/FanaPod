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
