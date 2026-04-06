```
     _          __  __      ___        _  _
    / \   _ __ \ \/ /     / _ \  _  _(_)| |_
   / _ \ | '__| \  / ___ | | | || || | || __|
  / ___ \| |    /  \|___|| |_| || || | || |_
 /_/   \_\_|   /_/\_\     \__\_\ \__,_|_| \__|
```

A terminal UI application for macOS that lists all running GUI applications and lets you quit them — gracefully or by force.

Built with Rust and [Ratatui](https://ratatui.rs/).

## Features

- Lists all running GUI applications with bundle IDs and PIDs
- **Graceful quit** — sends a quit command via AppleScript (like Command+Q)
- **Force quit** — sends SIGKILL to the process (like Force Quit dialog)
- **Multi-select** — select multiple apps and quit them all at once
- Confirmation dialog before any quit action
- Auto-refreshes the app list every 5 seconds
- Status messages with success/error feedback

## Installation

### Prerequisites

- macOS
- [Rust](https://rustup.rs/) (1.85+)

### Build from source

```bash
git clone https://git.cyberarx.systems/cyberarx/ArX-Quit.git
cd ArX-Quit
cargo build --release
```

The binary will be at `target/release/arx-quit`.

### Run directly

```bash
cargo run
```

## Keybindings

| Key | Action |
|---|---|
| `↑` / `k` | Move cursor up |
| `↓` / `j` | Move cursor down |
| `Space` | Toggle select/deselect app |
| `a` | Select all apps |
| `d` | Deselect all apps |
| `Enter` / `r` | Graceful quit (selected or cursor) |
| `f` | Force quit (selected or cursor) |
| `R` | Refresh app list |
| `q` | Exit ArX-Quit |

### Confirmation dialog

| Key | Action |
|---|---|
| `y` / `Enter` | Confirm quit |
| `n` / `Esc` | Cancel |

## How it works

1. **Listing apps** — Uses AppleScript via `osascript` to query System Events for all foreground (non-background) processes, retrieving names, bundle identifiers, and PIDs
2. **Graceful quit** — Sends `tell application "AppName" to quit` via AppleScript, allowing the app to save state and close cleanly
3. **Force quit** — Sends `kill -9 <PID>` to immediately terminate the process

## Project structure

```
src/
  main.rs      — Entry point, terminal setup/teardown, event loop
  app.rs       — Application state, message handling, key bindings
  ui.rs        — TUI layout and rendering (header, list, footer, dialogs)
  process.rs   — macOS process listing, graceful quit, force quit
```

## Dependencies

- [ratatui](https://crates.io/crates/ratatui) — Terminal UI framework
- [crossterm](https://crates.io/crates/crossterm) — Cross-platform terminal manipulation
- [anyhow](https://crates.io/crates/anyhow) — Error handling

## License

MIT
