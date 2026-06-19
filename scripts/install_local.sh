#!/usr/bin/env bash
# ==============================================================================
# install_local.sh — Install binary locally to ~/.cargo/bin/
# Run from root project: bash scripts/install_local.sh
# ==============================================================================
set -e

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BINARY_FILE="$ROOT_DIR/dist/comfyui-desktop"
DOWNLOADER_CLI="$ROOT_DIR/dist/comfyui-downloader-cli"
DOWNLOADER_TUI="$ROOT_DIR/dist/comfyui-downloader-tui"
INSTALL_DIR="${CARGO_HOME:-$HOME/.cargo}/bin"

echo "========================================"
echo " ComfyUI Desktop — Local Installer"
echo "========================================"
echo ""

if [ -f "$BINARY_FILE" ]; then
    echo "⚙️ Found compiled binary: $BINARY_FILE"
else
    echo "❌ Error: Compiled binary not found in $ROOT_DIR/dist/"
    echo "Please run the build script first:"
    echo "  bash scripts/build-launcher.sh   (for desktop app)"
    echo "  bash scripts/build-downloader.sh (for model downloader)"
    exit 1
fi

echo "[1/4] Preparing installation directories..."
mkdir -p "$INSTALL_DIR"
DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}"
mkdir -p "$DATA_DIR/applications"
mkdir -p "$DATA_DIR/icons/hicolor"

echo "[2/4] Installing binary..."
if ldd "$BINARY_FILE" >/dev/null 2>&1; then
  :
fi

INSTALLED_BINARY="$INSTALL_DIR/comfyui-desktop"
if [ -e "$INSTALLED_BINARY" ]; then
  if [ -e "/proc/$(pgrep -x -f '(^|/)(comfyui-desktop|comfyui-desktop$)' | tr '\n' ' ')" ] 2>/dev/null; then
    echo "  ❌ Installed launcher is currently running: $INSTALLED_BINARY"
    echo "     Close 'ComfyUI Desktop' before reinstalling, or stop it with:"
    echo "     pkill -x comfyui-desktop"
    exit 1
  fi
fi
cp "$BINARY_FILE" "$INSTALLED_BINARY"
chmod +x "$INSTALLED_BINARY"
echo "  ✅ Binary installed to: $INSTALLED_BINARY"

if [ -f "$DOWNLOADER_CLI" ]; then
    cp "$DOWNLOADER_CLI" "$INSTALL_DIR/comfyui-downloader-cli"
    chmod +x "$INSTALL_DIR/comfyui-downloader-cli"
    echo "  ✅ CLI downloader binary installed to: $INSTALL_DIR/comfyui-downloader-cli"
else
    echo "  ⚠️  CLI downloader binary not found in dist/. Skipping."
fi

if [ -f "$DOWNLOADER_TUI" ]; then
    cp "$DOWNLOADER_TUI" "$INSTALL_DIR/comfyui-downloader-tui"
    chmod +x "$INSTALL_DIR/comfyui-downloader-tui"
    echo "  ✅ TUI downloader binary installed to: $INSTALL_DIR/comfyui-downloader-tui"
else
    echo "  ⚠️  TUI downloader binary not found in dist/. Skipping."
fi

echo "[3/4] Installing application icons and generating desktop entries..."

# Install application icons from source assets
ICON_COUNT=0
# Prefer new location assets/icons/, fall back to old crates/launcher/assets/icons/
if [ -d "$ROOT_DIR/assets/icons" ]; then
    ICONS_SRC_DIR="$ROOT_DIR/assets/icons"
elif [ -d "$ROOT_DIR/crates/launcher/assets/icons" ]; then
    ICONS_SRC_DIR="$ROOT_DIR/crates/launcher/assets/icons"
else
    ICONS_SRC_DIR=""
fi

declare -A ICON_MAP=(
    ["32x32.png"]="32x32"
    ["64x64.png"]="64x64"
    ["128x128.png"]="128x128"
    ["128x128@2x.png"]="256x256"
    ["icon.png"]="512x512"
)

if [ -n "$ICONS_SRC_DIR" ]; then
    for icon_file in "${!ICON_MAP[@]}"; do
        size="${ICON_MAP[$icon_file]}"
        src_path="$ICONS_SRC_DIR/$icon_file"
        if [ -f "$src_path" ]; then
            dest_dir="$DATA_DIR/icons/hicolor/$size/apps"
            mkdir -p "$dest_dir"
            cp "$src_path" "$dest_dir/comfyui-desktop.png"
            echo "    ✅ Icon $size installed"
            ICON_COUNT=$((ICON_COUNT + 1))
        fi
    done
fi

if [ $ICON_COUNT -eq 0 ]; then
    echo "    ⚠️  No source icons found"
fi

# Generate desktop entry file
VERSION=$(grep -m1 '^version' "$ROOT_DIR/crates/launcher/Cargo.toml" | sed 's/.*"\(.*\)".*/\1/' || echo "0.5.0")
DEST_DESKTOP="$DATA_DIR/applications/comfyui-desktop.desktop"
cat > "$DEST_DESKTOP" <<EOF
[Desktop Entry]
Categories=Utility;
Comment=ComfyUI Desktop — Tauri v2 wrapper for ComfyUI with AMD ROCm GPU auto-configuration
Exec=$INSTALL_DIR/comfyui-desktop
Icon=comfyui-desktop
Name=ComfyUI Desktop
Terminal=false
Type=Application
Version=$VERSION
StartupWMClass=comfyui-desktop
EOF
echo "  ✅ Launcher desktop entry: $DEST_DESKTOP"

# Generate TUI downloader desktop entry
if [ -f "$DOWNLOADER_TUI" ]; then
    DEST_TUI_DESKTOP="$DATA_DIR/applications/comfyui-downloader.desktop"
    cat > "$DEST_TUI_DESKTOP" <<EOF
[Desktop Entry]
Categories=Utility;
Comment=ComfyUI Model Downloader — TUI to download and manage ComfyUI models
Exec=$INSTALL_DIR/comfyui-downloader-tui
Icon=comfyui-downloader
Name=ComfyUI Downloader
Terminal=true
Type=Application
EOF
    echo "  ✅ TUI downloader desktop entry: $DEST_TUI_DESKTOP"
    # Copy logo icon for downloader if present
    if [ -f "$ROOT_DIR/ComfyUI_Downloader.png" ]; then
        mkdir -p "$DATA_DIR/icons/hicolor/512x512/apps"
        cp "$ROOT_DIR/ComfyUI_Downloader.png" "$DATA_DIR/icons/hicolor/512x512/apps/comfyui-downloader.png"
        echo "  ✅ TUI downloader icon: 512x512"
    fi
fi

echo "[4/4] Updating desktop database..."

# Update system databases to immediately detect the new shortcut and icons
update-desktop-database "$DATA_DIR/applications" 2>/dev/null || true
gtk-update-icon-cache -f -t "$DATA_DIR/icons/hicolor" 2>/dev/null || true

echo ""
echo "========================================"
echo "✅ COMPLETE! ComfyUI Desktop is installed."
echo "Please search for 'ComfyUI Desktop' in your application menu"
echo "or run the following command in terminal:"
echo "  comfyui-desktop"
echo "========================================"
