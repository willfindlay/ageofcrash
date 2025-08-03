# Age of Crash Mouse Barrier - Development Guide

## Getting Started - READ FIRST

**IMPORTANT**: Before making any code changes, always inspect the project structure to understand the architecture:

1. **Inspect the main files**:
   - `ageofcrash-app/src/main.rs` - Contains the main message loop and application state
   - `mouse-barrier/src/lib.rs` - Core Windows hook implementation
   - `ageofcrash-app/src/hotkey.rs` - Hotkey detection patterns
   - `ageofcrash-app/src/config.rs` - Configuration management

2. **Understand the threading model**:
   - **Main thread**: Runs Windows message loop, handles hook installation/removal
   - **Background threads**: Used for monitoring (config watching, middle mouse detection)
   - **Hook callbacks**: Execute in hook thread context, must be fast
   - **Thread affinity**: Windows hooks must be managed from the main thread

3. **Key architectural patterns**:
   - Flag-based communication between threads using atomic variables
   - Hook operations processed in main message loop via `process_*_requests()` functions
   - State management through global static variables with proper synchronization
   - Event-driven design using Windows message pump

4. **Common pitfalls to avoid**:
   - Never install/uninstall hooks from background threads (causes deadlocks)
   - Don't perform blocking operations in hook callbacks
   - Always use atomic operations for cross-thread communication
   - Consider hook interference with game input systems

## Project Overview

Age of Crash Mouse Barrier is a Windows application that prevents the mouse cursor from entering a configurable rectangular area on the screen. It was specifically designed to work around a crash bug in Age of Empires IV that occurs when the mouse enters the bottom-left corner.

**Purpose:** Defensive tool to prevent game crashes caused by cursor position bugs
**Language:** Rust
**Platform:** Windows-only (uses Windows API hooks)
**Architecture:** Workspace with two crates (library + app)

## Project Structure

```
ageofcrash/
├── Cargo.toml              # Workspace configuration
├── Makefile               # Cross-compilation from WSL
├── README.md              # User documentation
├── config.ron             # Default configuration file
├── rust-analyzer.toml     # IDE configuration
├── ageofcrash-app/        # Main application crate
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs        # Entry point
│       ├── config.rs      # Configuration handling
│       └── hotkey.rs      # Hotkey management
└── mouse-barrier/         # Library crate for Windows hooks
    ├── Cargo.toml
    └── src/
        └── lib.rs         # Core barrier logic
```

## Key Dependencies

- **winapi**: Windows API bindings for low-level hooks
- **ron**: Rusty Object Notation for configuration
- **serde**: Serialization framework
- **tracing**: Structured logging

## Development Commands

### Building

```bash
# Native Windows build
cargo build --release

# Cross-compile from WSL (requires mingw-w64)
make build

# Run the application
cargo run --bin ageofcrash
```

### Deployment

```bash
# Deploy to Windows desktop
make deploy

# Alternative: deploy to C:\ageofcrash
make deploy-c
```

### Code Quality

```bash
# Format code
cargo fmt

# Run linter
cargo clippy

# Check compilation
cargo check
```

## Configuration System

The app uses RON (Rusty Object Notation) for configuration:
- **Location**: `config.ron` in the working directory
- **Auto-creation**: Creates default config if missing

### Config Structure
```ron
(
    hotkey: (
        ctrl: true,
        alt: false,
        shift: false,
        key: "F12",
    ),
    barrier: (
        x: 0,          // Left edge
        y: 1080,       // Bottom edge (bottom-left origin)
        width: 200,    // Extends right
        height: 40,    // Extends up
        push_factor: 50,
    ),
)
```

## Architecture Notes

1. **Two-crate design**:
   - `mouse-barrier`: Reusable library for Windows hooks
   - `ageofcrash-app`: Application logic and configuration

2. **Windows Hooks**:
   - Low-level mouse hook to intercept cursor movement
   - Keyboard hook for hotkey detection
   - Requires careful memory management

3. **Coordinate System**:
   - Uses bottom-left origin (mathematical style)
   - Y-coordinate represents bottom edge of barrier
   - Intuitive for bottom-screen UI elements

4. **Safety Considerations**:
   - Defensive tool only - prevents crashes
   - No data collection or network access
   - Requires admin privileges for system hooks

## Common Development Tasks

### Adding a new hotkey
1. Update `Config` struct in `config.rs`
2. Add key parsing logic in `hotkey.rs`
3. Update default configuration

### Modifying barrier behavior
1. Edit barrier logic in `mouse-barrier/src/lib.rs`
2. Test with different `push_factor` values
3. Consider edge cases (multi-monitor setups)

### Adding input monitoring (e.g., middle mouse detection)
**Example implementation**: Middle mouse monitoring for temporary barrier disable
1. **Background thread**: Use `GetAsyncKeyState` polling to detect input state
2. **Atomic flags**: Set `HOOK_*_REQUESTED` flags when state changes
3. **Main thread processing**: Add `process_*_requests()` function called from message loop
4. **Hook management**: Use same `install_*_hook()`/`uninstall_*_hook()` functions as hotkey system
5. **Pattern**: Never manage hooks from background threads - always use flag-based requests

### Debugging
- Enable trace logging: `RUST_LOG=trace cargo run`
- Check Windows Event Viewer for hook issues
- Use `cargo run -- --debug` flag if implemented

## Testing Notes

No automated tests currently exist. Manual testing procedure:
1. Build and run the application
2. Toggle barrier with configured hotkey
3. Test mouse movement near restricted area
4. Verify configuration hot-reload
5. Test on different screen resolutions

## Cross-compilation from WSL

The project includes WSL cross-compilation support:
- Uses `x86_64-pc-windows-gnu` target
- Requires `mingw-w64-gcc` toolchain
- Makefile automates build and deployment
- Deploys to Windows desktop with helper batch file

## Security Notes

This is a defensive security tool that:
- Prevents application crashes caused by UI bugs
- Uses system-level hooks responsibly
- Has no network connectivity
- Stores configuration locally only