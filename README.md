# Age of Crash Mouse Barrier

A Windows application that prevents your mouse cursor from entering a specific rectangular area on the screen, designed to work around the crash bug in Age of Empires IV when the mouse enters the bottom-left corner.

## Project Origin

This project started as a **vibe coding experiment** - coding with an AI assistant and just going with the vibe of whatever seemed interesting or fun to implement. No grand plan, no formal requirements, just "hey this gaming bug is annoying, let's see what happens if we try to fix it" and then following whatever direction felt right in the moment.

What began as casual experimentation somehow turned into a reasonably solid piece of software. Sometimes that's just how it goes.

## Features

- **Mouse Barrier**: Prevents the mouse from entering a configurable rectangular area
- **Push Factor**: When the mouse enters the restricted area, it gets pushed away by a configurable distance
- **Hotkey Toggle**: Toggle the barrier on/off using a configurable hotkey combination (F1-F12, A-Z, 0-9 with modifiers)
- **Real-time HUD**: Optional overlay showing barrier status, position, and mouse coordinates
- **Audio Feedback**: Configurable sound effects for barrier interactions
- **Hot Configuration Reload**: Automatically reloads settings when config file changes
- **RON Configuration**: Easy-to-edit configuration file format with smart defaults
- **Overlay Visualization**: Optional colored overlay showing the barrier area

## Building

This project requires Rust and is Windows-specific due to its use of Windows API hooks.

### Prerequisites
- Rust toolchain (latest stable)
- Windows 10 or later

### Native Windows Build
```bash
# Build the project on Windows
cargo build --release

# Run the application
cargo run --bin ageofcrash

# Run tests (79+ comprehensive unit tests)
cargo test

# Check code quality
cargo clippy
cargo fmt --check
```

### Cross-compilation from WSL

If you're developing in WSL and want to run the binary on Windows, you can cross-compile:

#### Setup for Arch Linux (WSL)
```bash
# Install basic build tools and cross-compilation dependencies
sudo pacman -S base-devel mingw-w64-gcc

# Add Windows target
rustup target add x86_64-pc-windows-gnu

# Verify the cross-compiler is available
which x86_64-w64-mingw32-gcc
```

#### Setup for Ubuntu/Debian (WSL)
```bash
# Install cross-compilation dependencies
sudo apt update && sudo apt install gcc-mingw-w64-x86-64

# Add Windows target
rustup target add x86_64-pc-windows-gnu
```

#### Build and Deploy
```bash
# Cross-compile for Windows
make build

# Build and copy to Windows desktop
make deploy

# Alternative: deploy to C:\ageofcrash
make deploy-c
```

The `make deploy` target will create a folder on your Windows desktop with:
- `ageofcrash.exe` - The executable
- `config.ron` - Configuration file
- `README.md` - This documentation
- `run.bat` - Easy-to-use batch file for running the app

## Configuration

The application reads its configuration from `config.ron` in the current directory. If the file doesn't exist, it will be created with default values.

**See `config.ron` for the complete configuration structure with detailed comments.**

### Configuration Options

- **hotkey**: Key combination to toggle the barrier
  - `ctrl`, `alt`, `shift`: Boolean values for modifier keys
  - `key`: The main key (supports F1-F12, A-Z, 0-9)

- **barrier**: Defines the restricted area using bottom-left origin
  - `x`: Left edge coordinate (grows rightward)
  - `y`: Bottom edge coordinate (this is the bottom of the barrier)
  - `width`: Width of barrier (extends right from x)
  - `height`: Height of barrier (extends upward from y)
  - `buffer_zone`: Additional detection area around the barrier (pixels)
  - `push_factor`: How far to push the cursor away when it enters the area
  - `overlay_color`: RGB color values (0-255) for barrier visualization
  - `overlay_alpha`: Transparency of overlay (0=invisible, 255=opaque)
  - `audio_feedback`: Optional sound file paths for barrier events

- **hud**: Real-time information overlay
  - `enabled`: Show/hide the HUD overlay
  - `position`: Screen corner placement (TopLeft, TopRight, BottomLeft, BottomRight)
  - `background_alpha`: HUD background transparency (0-255)

- **debug**: Enable detailed logging for troubleshooting

### Coordinate System

The barrier uses a bottom-left coordinate system (like math graphs):
- `(0, 1080)` = bottom-left corner of a 1080p screen
- `width` extends rightward from `x`
- `height` extends upward from `y`

This makes it intuitive to define UI panels that sit at the bottom of the screen.

## Usage

1. Configure the barrier area and hotkey in `config.ron`
2. Run the application
3. The barrier starts disabled - press your configured hotkey to enable it
4. Press the hotkey again to disable the barrier
5. Press Ctrl+C to exit the application

## Architecture

The project follows a clean two-crate workspace design:

- **mouse-barrier**: Reusable library crate providing Windows API hooks for mouse and keyboard interaction
- **ageofcrash-app**: Main application with configuration management, HUD, audio feedback, and user interface

### Key Components
- **Configuration System**: RON-based config with hot-reload and smart defaults
- **Windows Hooks**: Low-level mouse and keyboard event interception
- **HUD System**: Real-time overlay with position tracking and status display
- **Audio Feedback**: Optional sound effects for barrier interactions
- **Test Suite**: 79+ comprehensive unit tests covering all major functionality

## Testing & Quality Assurance

This project follows **Test-Driven Development (TDD)** principles with comprehensive test coverage:

### Test Coverage
- **79+ Unit Tests** across all components
- **Configuration Testing**: Serialization, validation, and merging
- **Core Logic Testing**: Geometry calculations, collision detection, state management
- **Integration Testing**: File watching, configuration reloading
- **Edge Case Testing**: Invalid inputs, error conditions, boundary values

### Continuous Integration
- **GitHub Actions** for automated testing and quality checks
- **Code Formatting**: Enforced via `cargo fmt`
- **Linting**: Zero-warning policy with `cargo clippy`
- **Multi-target Testing**: Debug and release builds on Windows runners

### Quality Commands
```bash
# Run all tests
cargo test

# Check code formatting
cargo fmt --check

# Run linter (zero warnings required)
cargo clippy -- -D warnings

# Check compilation
cargo check --all-targets --all-features
```

## Contributing

This project welcomes contributions! Before contributing:

1. **Read the Development Guide**: See `CLAUDE.md` for detailed development workflows, architecture notes, and best practices
2. **Follow TDD**: Write tests first when adding new functionality
3. **Maintain Quality**: All code must pass `cargo clippy`, `cargo fmt --check`, and `cargo test`
4. **Update Documentation**: Keep both `README.md` and `CLAUDE.md` current with your changes

### Development Workflow
```bash
# 1. Install dependencies and setup
rustup target add x86_64-pc-windows-gnu  # If cross-compiling

# 2. Make changes following TDD principles
cargo test  # Run tests frequently

# 3. Quality checks before committing
cargo clippy -- -D warnings  # Zero warnings required
cargo fmt                    # Format code
cargo test                   # All tests must pass

# 4. Update documentation as needed
```

## Requirements

- **Windows 10 or later** (uses Windows API)
- **Rust toolchain** (latest stable)

## Safety and Security

This application uses low-level Windows hooks that can potentially interfere with system operation. The software is designed as a **defensive tool** to prevent application crashes and does not collect data or access the network.

**Security Features:**
- No network connectivity
- Local configuration storage only
- Comprehensive test coverage for reliability
- Open source for transparency and security auditing

Use responsibly and only run from trusted sources.