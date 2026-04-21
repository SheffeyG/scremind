# scremind - Agent Guide

## Project Overview

scremind is a Windows-only screen overlay reminder application written in Rust. It displays a fullscreen semi-transparent overlay at configurable intervals or scheduled times to remind the user to take breaks. The application runs as a system tray icon with no console window.

## Build & Run

```powershell
# Build (release recommended for production)
cargo build --release

# Check and run (debug) do not use 'cargo check'
cargo run
```

The build uses `embed-resource` to embed the icon from `assets/app.rc` into the executable. The `#![windows_subsystem = "windows"]` attribute suppresses the console window.

## Project Structure

```
scremind/
├── assets/
│   ├── app.ico          # Application icon
│   └── app.rc           # Windows resource file (icon embedding)
├── src/
│   ├── main.rs          # Entry point: logger, config, tray window, message loop
│   ├── config.rs        # TOML config loading with serde defaults
│   ├── timer.rs         # Interval & schedule reminder logic, tick-driven state machine
│   ├── overlay.rs       # Fullscreen layered overlay window with fade animation
│   ├── tray.rs          # System tray icon, context menu, window procedure
│   └── autostart.rs     # Windows Startup folder shortcut management via PowerShell
├── build.rs             # Build script: compiles app.rc via embed-resource
├── Cargo.toml
└── config.toml          # User config (gitignored, auto-generated on first run)
```

## Architecture

### Autostart

Creates/removes a `.lnk` shortcut in `%APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup` via PowerShell COM automation.

## Notes for Agents

- This project targets **Windows only**. Do not introduce cross-platform abstractions.
- The `windows` crate feature flags are in `Cargo.toml`. Add new Win32 features there if needed.
- Config struct fields use `#[serde(default)]` with standalone default functions. Follow this pattern when adding new config fields.
- The overlay uses double buffering (memory DC + BitBlt) to prevent flickering. Maintain this when modifying paint logic.
- Global mutable state uses `Mutex` or `AtomicBool`. Be mindful of lock ordering to avoid deadlocks.
- The `get_current_time()` in `timer.rs` hardcodes UTC+8 offset. This is intentional for the author's timezone.
