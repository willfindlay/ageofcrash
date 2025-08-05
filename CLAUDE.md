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
   - **CRITICAL**: Never use hardcoded screen dimensions, resolutions, or coordinate values in production code
   - Always detect screen metrics dynamically using appropriate WinAPI calls (`GetSystemMetrics`, `EnumDisplaySettings`, etc.)
   - **CRITICAL DPI SCALING**: Windows APIs mix coordinate systems - some use logical DPI-scaled coordinates while others use physical pixels. Must convert between them to avoid positioning bugs (see DPI Scaling section below)

5. **Consult the git commit history**:
   - The commit history is an **excellent resource** for understanding project evolution and context
   - Each commit includes detailed technical explanations, implementation decisions, and known limitations
   - Use `git log --oneline` for quick overview, `git show <commit>` for detailed information
   - Commits follow Conventional Commits format with comprehensive technical details
   - Search commit messages: `git log --grep="keyword"` to find related changes
   - Understanding previous solutions helps avoid repeating past mistakes and builds on existing patterns

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
├── rust-toolchain.toml    # Rust version pinning for consistency
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
# Format code (all packages)
cargo fmt --all

# Run linter (with all targets and features)
cargo clippy --all-targets --all-features

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

### CRITICAL: Code Quality Checks After EVERY Change

**MANDATORY**: After making ANY code change, no matter how small, you MUST run:

```bash
# 1. Run linter to catch code issues (exact CI command)
cargo clippy --all-targets --all-features -- -D warnings

# 2. Fix any clippy warnings/errors identified
# Use cargo clippy --fix when possible, or manually address each issue

# 3. Format code consistently (all packages)
cargo fmt --all
```

**BEFORE COMMITTING**: The above checks are ABSOLUTELY REQUIRED. Additionally run:

```bash
# 4. Run all tests to ensure functionality
cargo test

# 5. Deploy to verify the complete build process (optional but recommended)
make deploy
```

This ensures:
- Code follows Rust best practices (`clippy`) with **all lints addressed**
- Consistent formatting across the codebase (`fmt`)
- All functionality works as expected (`test`)
- The complete build and deployment pipeline works (`deploy`)

**CRITICAL REMINDERS**:
- **NEVER commit without running `cargo clippy --all-targets --all-features -- -D warnings` and `cargo fmt`** - CI will fail if you do
- **NEVER ignore clippy warnings** - fix ALL lints before proceeding
- **Run these checks after EVERY change** - even small ones like removing a function or dependency
- **The order matters**: Run clippy first to fix issues, then fmt to ensure consistent formatting
- **Use exact CI command**: `--all-targets --all-features -- -D warnings` checks all code including tests and examples

### Continuous Integration

The project includes GitHub Actions workflows for automated quality assurance:

**Unit Tests Workflow** (`.github/workflows/tests.yml`):
- Runs on push/pull request to main/develop branches (skips documentation-only changes)
- Executes all 79+ unit tests in both debug and release modes
- Generates test coverage reports and uploads results as artifacts
- Uses Windows runners to match the target platform

**Code Quality Workflow** (`.github/workflows/code-quality.yml`):
- Runs on push/pull request to main/develop branches (skips documentation-only changes)
- Enforces code formatting with `cargo fmt --check`
- Runs clippy lints with warnings treated as errors (`-D warnings`)
- Verifies compilation in both debug and release modes
- Validates project structure and required files
- Caches dependencies for faster builds

**CI Optimizations**: 
- Both workflows use `paths-ignore` to skip running when only documentation files (*.md, *.txt, docs/, LICENSE) are changed, saving CI resources
- Rust toolchain version is managed via `rust-toolchain.toml` to ensure consistency between local development and CI environments

Both workflows must pass before merging changes to ensure consistent code quality and functionality.

### Clippy Lint Guidelines

When fixing clippy lints, follow these patterns:

- **Type complexity**: Extract complex types into type aliases
- **Too many arguments**: Use a separate Conf struct for constructs with too many paramters
- **Manual clamp**: Replace `.max().min()` chains with `.clamp()`
- **Clone on copy**: Use dereference (`*`) instead of `.clone()` for Copy types
- **Needless borrow**: Remove unnecessary `&` references

### Commit Message Style Guide

**CRITICAL**: This project follows [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/) specification for all commit messages. This ensures consistent, parseable commit history and enables automated versioning.

#### Commit Message Structure

```
<type>[optional scope]: <description>

[optional body]

[optional footer(s)]
```

#### Commit Types (REQUIRED)

**Primary Types:**
- `feat:` - New feature (correlates with MINOR version bump)
- `fix:` - Bug fix (correlates with PATCH version bump)

**Additional Types:**
- `docs:` - Documentation changes only
- `style:` - Code style changes (formatting, missing semicolons, etc.)
- `refactor:` - Code refactoring without changing functionality
- `test:` - Adding or updating tests
- `perf:` - Performance improvements
- `build:` - Build system or dependency changes
- `ci:` - CI/CD configuration changes
- `chore:` - Maintenance tasks, dependency updates

#### Breaking Changes

Indicate breaking changes with either:
1. `!` after the type/scope: `feat(api)!: remove deprecated endpoint`
2. `BREAKING CHANGE:` footer in the commit body

Breaking changes correlate with MAJOR version bumps.

#### Formatting Rules

**Title Line (REQUIRED):**
- **Maximum 50 characters** - titles longer than 50 chars will be rejected
- Use lowercase for type and description
- No period at the end
- Use imperative mood ("add feature" not "added feature")

**Body (OPTIONAL):**
- Separate from title with blank line
- **Word wrap prose at 72 characters** for readability in terminals
- Explain the "what" and "why", not the "how"
- Use present tense, imperative mood
- **For major features or bug fixes**: Include technical details about
  how the implementation works, any limitations or pitfalls, and
  potential future improvements or considerations
- Long code examples should use indented code blocks:

    ```rust
    // Example code that exceeds line length
    let very_long_variable_name = some_function_call()
        .with_method_chaining()
        .and_more_calls();
    ```

**Footer (OPTIONAL):**
- Reference issues: `Closes #123` or `Fixes #456`
- Breaking changes: `BREAKING CHANGE: description`
- Co-authored commits (see below)

#### Scopes (OPTIONAL)

Use scopes to indicate which part of the codebase is affected:
- `feat(config): add new barrier shape options`
- `fix(hooks): resolve cursor positioning on high-DPI displays`
- `docs(readme): update installation instructions`

Common scopes for this project:
- `config` - Configuration system changes
- `hooks` - Windows hook implementation
- `barrier` - Core barrier logic
- `hotkey` - Hotkey detection system
- `hud` - HUD display system
- `overlay` - Visual overlay windows
- `tests` - Test suite modifications
- `ci` - Continuous integration

#### Security and Privacy (CRITICAL)

**NEVER include in commit messages:**
- API keys, tokens, or passwords
- Private credentials of any kind
- Personal information (addresses, phone numbers, etc.)
- Internal URLs or system paths that could expose infrastructure
- Database connection strings or configuration secrets
- AI attribution lines such as "Generated with Claude" or "Co-Authored-By: Claude"

**Exception**: The `Signed-off-by` line is the ONLY personal information
that should appear in commit messages, and only the name/email from
git config.

**If examples are needed**: Use sanitized, fictional examples instead
of real credentials. For instance, use `api_key: "example_key_123"` or
`host: "example.com"` rather than actual values.

#### Commit Sign-off (REQUIRED for Claude Code)

**IMPORTANT**: All commits must be signed off with the human coder's 
information from git config. Always use the `-s` flag with `git commit`:

```bash
git commit -s -m "commit message"
```

The `-s` flag automatically adds the `Signed-off-by` line using the name
and email from your git configuration. This acknowledges that the human 
developer supervised and approved the AI-generated changes according to 
project contribution guidelines.

#### Example Commit Messages

**Simple feature:**
```
feat: add barrier color customization

Allow users to configure barrier overlay color via config.ron.
Supports RGB values and transparency settings.

Closes #42
Signed-off-by: [Name] <[email]>
```

**Bug fix with breaking change:**
```
fix(hooks)!: correct DPI scaling coordinate conversion

Previously mixed physical and logical coordinates causing incorrect
cursor positioning on high-DPI displays. This changes the internal
coordinate system to consistently use physical coordinates.

BREAKING CHANGE: MouseBarrierConfig now expects physical pixel
coordinates instead of logical DPI-scaled coordinates.

Fixes #89
Signed-off-by: [Name] <[email]>
```

**Documentation update:**
```
docs: add DPI scaling troubleshooting guide

Explain coordinate system differences and common positioning bugs
to help future developers avoid DPI scaling issues.

Signed-off-by: [Name] <[email]>
```

**Multi-line code example in body:**
```
refactor(barrier): simplify collision detection logic

Extract complex geometry calculations into helper functions for
better readability and testability:

    fn point_in_rect(point: &POINT, rect: &RECT) -> bool {
        point.x >= rect.left && point.x < rect.right && 
        point.y >= rect.top && point.y < rect.bottom
    }

This change maintains identical behavior while improving code
organization and making unit testing easier.

Signed-off-by: [Name] <[email]>
```

**Major feature with technical details:**
```
feat(hooks): implement predictive cursor blocking

Add algorithm to detect fast mouse movements that might skip
over traditional collision detection by sampling intermediate
points along the movement path.

Implementation uses linear interpolation to check 10 sample
points between previous and current cursor positions. When
any sample point intersects the barrier, cursor is redirected
to the last safe position outside the buffer zone.

Limitations: Very high DPI mice (>12000 DPI) at maximum
sensitivity may still occasionally skip detection with
extremely fast movements. Algorithm adds ~0.1ms latency
to mouse processing in barrier-enabled areas.

Future work: Consider using Bresenham's line algorithm for
more efficient path sampling or implementing hardware-level
cursor interception.

Signed-off-by: [Name] <[email]>
```

#### Tools and Validation

- Use `git log --oneline` to review commit title lengths
- Configure git hook to validate commit message format if desired
- Claude Code will automatically check title length and format

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

## DPI Scaling and Coordinate Systems

**CRITICAL**: Windows uses different coordinate systems that can cause subtle bugs if not handled correctly:

### Coordinate System Types

1. **Physical Coordinates** (used internally by mouse-barrier):
   - Raw screen pixels from `EnumDisplaySettings`
   - Examples: 3840x2160 on a 4K monitor
   - Used for barrier collision detection and internal calculations

2. **Logical Coordinates** (DPI-scaled):
   - From `GetSystemMetrics(SM_CXSCREEN/SM_CYSCREEN)`
   - Scaled by Windows DPI settings (e.g., 1920x1080 at 200% scaling)
   - Used by `SetCursorPos` and some other Windows APIs

### Implementation Details (`mouse-barrier/src/lib.rs`)

The codebase caches both coordinate systems on initialization:

```rust
// Logical (DPI-scaled) coordinates
let width = GetSystemMetrics(SM_CXSCREEN);
let height = GetSystemMetrics(SM_CYSCREEN);

// Physical coordinates
let mut dev_mode: DEVMODEW = std::mem::zeroed();
if EnumDisplaySettingsW(std::ptr::null(), ENUM_CURRENT_SETTINGS, &mut dev_mode) != 0 {
    physical_width = dev_mode.dmPelsWidth as i32;
    physical_height = dev_mode.dmPelsHeight as i32;
}
```

### Conversion Functions

**Physical → Logical** (for `SetCursorPos` and Windows API calls):
```rust
let scale_x = logical_width as f64 / physical_width as f64;
let scale_y = logical_height as f64 / physical_height as f64;
let logical_x = (physical_x as f64 * scale_x).round() as i32;
let logical_y = (physical_y as f64 * scale_y).round() as i32;
```

**Critical Usage Locations**:
- `push_point_out_of_rect()`: Converts barrier positions before calling `SetCursorPos`
- `create_overlay_windows()`: Scales barrier dimensions for window positioning
- Mouse hook receives physical coordinates, internal calculations use physical, then convert to logical for cursor movement

### Common DPI Scaling Bugs to Avoid

1. **Mixed coordinate systems**: Never mix physical coordinates from mouse hooks with logical coordinates from `GetSystemMetrics`
2. **Assuming 1:1 scaling**: DPI scaling can be 125%, 150%, 200%, or custom values
3. **Hardcoded positions**: Always calculate positions dynamically using proper scaling
4. **API inconsistency**: Some Windows APIs expect logical coordinates, others expect physical

### When DPI Scaling Matters

- **High DPI displays**: 4K monitors with Windows scaling (common scenario)
- **Multi-monitor setups**: Different monitors may have different DPI settings
- **Remote desktop**: RDP sessions can have different scaling than local display
- **Accessibility**: Users with vision impairments often use high DPI scaling

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

## Logging Standards

**IMPORTANT**: All logging in this project must use the `tracing` crate with appropriate log levels:

- **`error!`**: Critical errors that prevent normal operation (hook failures, configuration errors)
- **`warn!`**: Important warnings that don't stop execution but indicate potential issues
- **`info!`**: General information about application state changes (barrier enabled/disabled, screen metrics)
- **`debug!`**: Detailed debugging information useful during development
- **`trace!`**: Very verbose logging for deep debugging (coordinate calculations, hook callbacks)

**Never use**:
- `println!` or `eprintln!` for logging in production code
- `dbg!` macro (acceptable only for temporary debugging during development)
- Any other logging mechanisms

**Pattern**: Use structured logging with context when possible:
```rust
info!("Screen metrics initialized - Logical: {}x{}, Physical: {}x{}", 
      width, height, physical_width, physical_height);
```

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