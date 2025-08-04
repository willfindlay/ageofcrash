# Age of Crash Mouse Barrier Makefile

# Configuration
WINDOWS_TARGET = x86_64-pc-windows-gnu
BINARY_NAME = ageofcrash
WINDOWS_DEPLOY_DIR = /mnt/c/Users/$(shell cmd.exe /c "echo %USERNAME%" 2>/dev/null | tr -d '\r')/Desktop/ageofcrash
BUILD_DIR = target/$(WINDOWS_TARGET)/release

# Platform Detection
IS_WSL := $(shell if [ -f /proc/version ] && grep -qi microsoft /proc/version; then echo "true"; else echo "false"; fi)
IS_WINDOWS := $(shell if [ "$(OS)" = "Windows_NT" ] || command -v cmd.exe >/dev/null 2>&1; then echo "true"; else echo "false"; fi)
IS_LINUX := $(shell if [ "$(shell uname -s)" = "Linux" ] && [ "$(IS_WSL)" = "false" ]; then echo "true"; else echo "false"; fi)

.PHONY: help build deploy clean run

help:
	@echo "Age of Crash Mouse Barrier Build Targets:"
	@echo ""
ifeq ($(IS_WSL),true)
	@echo "  build   - Cross-compile for Windows"
	@echo "  deploy  - Build and copy files to Windows desktop"
	@echo "  run     - Deploy and run application on Windows"
	@echo "  clean   - Clean build artifacts"
	@echo ""
	@echo "Running in WSL - cross-compilation with Windows deployment"
	@echo "See README.md for setup instructions for your Linux distribution"
else ifeq ($(IS_WINDOWS),true)
	@echo "  build   - Build application natively"
	@echo "  deploy  - Build application (deployment not needed on native platform)"
	@echo "  run     - Build and run application"
	@echo "  clean   - Clean build artifacts"
	@echo ""
	@echo "Running on Windows - native build"
else ifeq ($(IS_LINUX),true)
	@echo "  build   - Cross-compile for Windows"
	@echo "  deploy  - Build application (no deployment - cross-compile only)"
	@echo "  run     - Cross-compile only (cannot run Windows binary on Linux)"
	@echo "  clean   - Clean build artifacts"
	@echo ""
	@echo "Running on Linux - cross-compilation only"
	@echo "See README.md for mingw-w64 setup instructions"
else
	@echo "  build   - Build application"
	@echo "  deploy  - Build application"
	@echo "  run     - Build application"
	@echo "  clean   - Clean build artifacts"
	@echo ""
	@echo "Platform detection uncertain - using default behavior"
endif

build:
ifeq ($(IS_WINDOWS),true)
	@echo "Building natively for Windows..."
	cargo build --release
else
	@echo "Cross-compiling for Windows..."
	cargo build --release --target $(WINDOWS_TARGET)
endif

deploy: build
ifeq ($(IS_WSL),true)
	@echo "Deploying to Windows desktop..."
	@# Create deployment directory
	mkdir -p "$(WINDOWS_DEPLOY_DIR)"
	
	@# Copy binary
	cp "$(BUILD_DIR)/$(BINARY_NAME).exe" "$(WINDOWS_DEPLOY_DIR)/"
	
	@# Copy config file only if it doesn't exist
	test -f "$(WINDOWS_DEPLOY_DIR)/config.ron" || cp config.ron "$(WINDOWS_DEPLOY_DIR)/"
	
	@# Copy README
	cp README.md "$(WINDOWS_DEPLOY_DIR)/"
	
	@# Create a simple batch file to run the application
	echo '@echo off' > "$(WINDOWS_DEPLOY_DIR)/run.bat"
	echo 'echo Starting Age of Crash Mouse Barrier...' >> "$(WINDOWS_DEPLOY_DIR)/run.bat"
	echo 'echo Press Ctrl+F12 (default) to toggle the mouse barrier' >> "$(WINDOWS_DEPLOY_DIR)/run.bat"
	echo 'echo Press Ctrl+C to exit' >> "$(WINDOWS_DEPLOY_DIR)/run.bat"
	echo 'echo.' >> "$(WINDOWS_DEPLOY_DIR)/run.bat"
	echo '$(BINARY_NAME).exe' >> "$(WINDOWS_DEPLOY_DIR)/run.bat"
	echo 'pause' >> "$(WINDOWS_DEPLOY_DIR)/run.bat"
	
	@echo ""
	@echo "✓ Deployment complete!"
	@echo "Files copied to: $(WINDOWS_DEPLOY_DIR)"
	@echo ""
	@echo "To run on Windows:"
	@echo "  1. Navigate to your Desktop/ageofcrash folder"
	@echo "  2. Double-click run.bat or run ageofcrash.exe directly"
	@echo "  3. Edit config.ron to customize settings"
else ifeq ($(IS_WINDOWS),true)
	@echo "✓ Build complete!"
	@echo "Application built natively - no deployment needed."
	@echo "Run 'make run' or execute ./target/release/$(BINARY_NAME).exe directly"
else
	@echo "✓ Cross-compilation complete!"
	@echo "Windows binary created at: $(BUILD_DIR)/$(BINARY_NAME).exe"
	@echo "Copy binary to Windows system to run."
endif

run:
ifeq ($(IS_WSL),true)
	@echo "Deploying and running on Windows..."
	$(MAKE) deploy
	@echo ""
	@echo "Starting application in new Windows terminal..."
	@echo "To exit: Close the terminal window or use Ctrl+C in the Windows terminal"
	@echo "Application hotkey: Ctrl+F12 (default) to toggle barrier"
	@echo ""
	@# Try Windows Terminal first, fall back to cmd if not available
	@if command -v wt.exe >/dev/null 2>&1; then \
		wt.exe -d "$(subst /mnt/c,C:,$(WINDOWS_DEPLOY_DIR))" cmd /k "$(BINARY_NAME).exe"; \
	else \
		cmd.exe /c "start \"Age of Crash\" cmd /k \"cd /d $(subst /mnt/c,C:,$(WINDOWS_DEPLOY_DIR)) && $(BINARY_NAME).exe\""; \
	fi
else ifeq ($(IS_WINDOWS),true)
	@echo "Building and running natively..."
	$(MAKE) build
	@echo "Starting application..."
	./target/release/$(BINARY_NAME).exe
else
	@echo "Cannot run Windows binary on this platform."
	@echo "Cross-compiling for Windows only..."
	$(MAKE) build
	@echo "Binary created at: $(BUILD_DIR)/$(BINARY_NAME).exe"
	@echo "Copy to Windows system to run."
endif

clean:
	cargo clean
ifeq ($(IS_WSL),true)
	rm -rf "$(WINDOWS_DEPLOY_DIR)"
endif

# Note: Run 'make deploy' after setting up dependencies (see README.md)