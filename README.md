# Age of Crash Mouse Barrier

A Windows application that prevents your mouse cursor from entering a specific rectangular area on the screen, designed to work around the crash bug in Age of Empires IV when the mouse enters the bottom-left corner.

## Features

- **Mouse Barrier**: Prevents the mouse from entering a configurable rectangular area
- **Push Factor**: When the mouse enters the restricted area, it gets pushed away by a configurable distance
- **Hotkey Toggle**: Toggle the barrier on/off using a configurable hotkey combination
- **RON Configuration**: Easy-to-edit configuration file format

## Building

This project requires Rust and is Windows-specific due to its use of Windows API hooks.

### Native Windows Build
```bash
# Build the project on Windows
cargo build --release

# Run the application
cargo run --bin ageofcrash
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

### Default Configuration

```ron
(
    hotkey: (
        ctrl: true,
        alt: false,
        shift: false,
        key: "F12",
    ),
    barrier: (
        x: 0,          // Left edge (x grows right)
        y: 1080,       // Bottom edge (y is bottom of barrier)
        width: 200,    // Width grows right from x
        height: 40,    // Height grows up from y
        push_factor: 50,
    ),
)
```

### Configuration Options

- **hotkey**: Key combination to toggle the barrier
  - `ctrl`, `alt`, `shift`: Boolean values for modifier keys
  - `key`: The main key (supports F1-F12, A-Z, 0-9)

- **barrier**: Defines the restricted area using bottom-left origin
  - `x`: Left edge coordinate (grows rightward)
  - `y`: Bottom edge coordinate (this is the bottom of the barrier)
  - `width`: Width of barrier (extends right from x)
  - `height`: Height of barrier (extends upward from y)
  - `push_factor`: How far to push the cursor away when it enters the area

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

The project consists of two crates:

- **mouse-barrier**: Library crate providing Windows API hooks for mouse and keyboard
- **ageofcrash-app**: Main application with configuration and user interface

## Requirements

- Windows (uses Windows API)
- Rust toolchain
- Administrator privileges may be required for low-level hooks

## Safety and Security

This application uses low-level Windows hooks that can potentially interfere with system operation. Use responsibly and only run from trusted sources.