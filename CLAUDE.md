# Age of Crash Mouse Barrier - Development Guide

**CRITICAL**: This file serves as the primary reference for all development work on this project. **Always update CLAUDE.md** when:
- Making major architectural changes or introducing new features
- Fixing significant bugs or resolving complex issues
- Modifying the development workflow, build process, or testing procedures
- Adding new dependencies, tools, or development practices
- Discovering important patterns, gotchas, or best practices

**IMPORTANT**: Also **keep README.md updated** with user-facing changes and critical information that new developers need to contribute to the project. The README serves as the public face of the project while CLAUDE.md contains detailed development guidance.

Keeping both documentation files current ensures future AI assistants and developers have accurate, up-to-date guidance for working with this codebase effectively.

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

# Run tests
cargo test
```

## Development Workflow

**IMPORTANT**: This project follows **Test-Driven Development (TDD)** principles wherever possible and feasible. When implementing new features or making changes:

1. **Write tests first** when adding new functionality
2. **Run tests frequently** during development to catch regressions early
3. **Maintain comprehensive test coverage** for all critical components
4. **Use tests to guide implementation** and ensure code meets requirements

The current test suite includes 79+ comprehensive unit tests covering:
- Configuration structs and serialization
- Mouse barrier core functionality and geometry calculations  
- Hotkey detection and state management
- HUD positioning and rendering logic
- File watching and configuration reloading
- Error handling and edge cases

**IMPORTANT**: After completing any requested code change, always run this full verification sequence:

```bash
# 1. Run linter to catch code issues
cargo clippy

# 2. Fix any clippy warnings/errors identified
# Use cargo clippy --fix when possible, or manually address each issue

# 3. Format code consistently
cargo fmt

# 4. Run all tests to ensure functionality
cargo test

# 5. Deploy to verify the complete build process
make deploy
```

This ensures:
- Code follows Rust best practices (`clippy`) with **all lints addressed**
- Consistent formatting across the codebase (`fmt`)
- All functionality works as expected (`test`)
- The complete build and deployment pipeline works (`deploy`)

**Note**: Never leave clippy warnings unaddressed. Fix all lints before proceeding to the next step.

### Continuous Integration

The project includes GitHub Actions workflows for automated quality assurance:

**Unit Tests Workflow** (`.github/workflows/tests.yml`):
- Runs on every push and pull request to main/develop branches
- Executes all 79+ unit tests in both debug and release modes
- Generates test coverage reports and uploads results as artifacts
- Uses Windows runners to match the target platform

**Code Quality Workflow** (`.github/workflows/code-quality.yml`):
- Enforces code formatting with `cargo fmt --check`
- Runs clippy lints with warnings treated as errors (`-D warnings`)
- Verifies compilation in both debug and release modes
- Validates project structure and required files
- Caches dependencies for faster builds

Both workflows must pass before merging changes to ensure consistent code quality and functionality.

### Clippy Lint Guidelines

When fixing clippy lints, follow these patterns:

- **Type complexity**: Extract complex types into type aliases
- **Too many arguments**: Use a separate Conf struct for constructs with too many paramters
- **Manual clamp**: Replace `.max().min()` chains with `.clamp()`
- **Clone on copy**: Use dereference (`*`) instead of `.clone()` for Copy types
- **Needless borrow**: Remove unnecessary `&` references

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

## Testing Strategy

**Test-Driven Development**: This project follows TDD principles with comprehensive unit test coverage.

### Automated Testing

The project includes **79+ unit tests** covering all major components:

**Configuration Testing** (25 tests):
- Config struct creation and validation
- Serialization/deserialization (RON and JSON)
- Configuration merging with defaults
- Virtual key code parsing and validation
- Error handling for invalid configurations

**Mouse Barrier Core** (10 tests):
- Geometric calculations and collision detection
- Coordinate system conversion (Windows ↔ mathematical)
- Push factor calculations and boundary handling
- Color conversion and state management

**Hotkey Detection** (19 tests):
- Key combination detection and modifier handling
- State management and configuration updates
- Edge cases for invalid keys and combinations

**HUD System** (10 tests):
- Position calculations for all screen corners
- Color constants and rendering logic
- Mouse position tracking and barrier detection

**File Watching** (13 tests):
- Configuration file monitoring and hot-reload
- Error handling for invalid file modifications
- Thread management and cleanup

**Testing Commands**:
```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run specific test module
cargo test config::tests
```

### Manual Testing Procedure
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