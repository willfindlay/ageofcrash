# Age of Crash Mouse Barrier Makefile

# Configuration
WINDOWS_TARGET = x86_64-pc-windows-gnu
BINARY_NAME = ageofcrash
WINDOWS_DEPLOY_DIR = /mnt/c/Users/$(shell cmd.exe /c "echo %USERNAME%" 2>/dev/null | tr -d '\r')/Desktop/ageofcrash
BUILD_DIR = target/$(WINDOWS_TARGET)/release

.PHONY: help build deploy clean

help:
	@echo "Age of Crash Mouse Barrier Build Targets:"
	@echo ""
	@echo "  build   - Cross-compile for Windows"
	@echo "  deploy  - Build and copy files to Windows desktop"
	@echo "  clean   - Clean build artifacts"
	@echo ""
	@echo "See README.md for setup instructions for your Linux distribution"

build:
	@echo "Cross-compiling for Windows..."
	cargo build --release --target $(WINDOWS_TARGET)

deploy: build
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
	echo 'echo Press Ctrl+F12 to toggle the mouse barrier' >> "$(WINDOWS_DEPLOY_DIR)/run.bat"
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

clean:
	cargo clean
	rm -rf "$(WINDOWS_DEPLOY_DIR)"

# Note: Run 'make deploy' after setting up dependencies (see README.md)

# Alternative deploy location (if desktop doesn't work)
deploy-c: build
	@echo "Deploying to C:/ageofcrash..."
	mkdir -p /mnt/c/ageofcrash
	cp "$(BUILD_DIR)/$(BINARY_NAME).exe" /mnt/c/ageofcrash/
	test -f /mnt/c/ageofcrash/config.ron || cp config.ron /mnt/c/ageofcrash/
	cp README.md /mnt/c/ageofcrash/
	echo '@echo off' > /mnt/c/ageofcrash/run.bat
	echo 'echo Starting Age of Crash Mouse Barrier...' >> /mnt/c/ageofcrash/run.bat
	echo '$(BINARY_NAME).exe' >> /mnt/c/ageofcrash/run.bat
	echo 'pause' >> /mnt/c/ageofcrash/run.bat
	@echo "✓ Files deployed to C:/ageofcrash/"