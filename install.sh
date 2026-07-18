#!/bin/bash
set -e

BINARY="target/release/yumic-server"
APP_NAME="yumic"
INSTALL_DIR="$HOME/.local/bin"
DESKTOP_DIR="$HOME/.local/share/applications"
ICON_DIR="$HOME/.local/share/icons/hicolor/scalable/apps"

echo "Installing YuMic..."

# Build release binary
if [ ! -f "$BINARY" ]; then
    echo "Building release binary..."
    cargo build --release
fi

# Create directories
mkdir -p "$INSTALL_DIR"
mkdir -p "$DESKTOP_DIR"
mkdir -p "$ICON_DIR"

# Install binary
cp "$BINARY" "$INSTALL_DIR/yumic-server"
chmod +x "$INSTALL_DIR/yumic-server"
echo "  Binary: $INSTALL_DIR/yumic-server"

# Install desktop file
cp assets/yumic.desktop "$DESKTOP_DIR/yumic.desktop"
echo "  Desktop: $DESKTOP_DIR/yumic.desktop"

# Install icon
cp assets/yumic.svg "$ICON_DIR/yumic.svg"
echo "  Icon: $ICON_DIR/yumic.svg"

# Update icon cache
gtk-update-icon-cache "$HOME/.local/share/icons/hicolor" 2>/dev/null || true

# Update desktop database
update-desktop-database "$DESKTOP_DIR" 2>/dev/null || true

echo ""
echo "Installed! You can now:"
echo "  - Find 'YuMic' in your application launcher"
echo "  - Run from terminal: yumic-server"
echo "  - Uninstall: ./uninstall.sh"
