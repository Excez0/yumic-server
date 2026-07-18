#!/bin/bash
echo "Uninstalling YuMic..."

rm -f "$HOME/.local/bin/yumic-server"
rm -f "$HOME/.local/share/applications/yumic.desktop"
rm -f "$HOME/.local/share/icons/hicolor/scalable/apps/yumic.svg"

gtk-update-icon-cache "$HOME/.local/share/icons/hicolor" 2>/dev/null || true
update-desktop-database "$HOME/.local/share/applications" 2>/dev/null || true

echo "Uninstalled."
