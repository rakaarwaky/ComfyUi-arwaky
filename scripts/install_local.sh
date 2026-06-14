#!/usr/bin/env bash
# ==============================================================================
# install_local.sh — Install binary locally to ~/.local/
# Run from root project: bash scripts/install_local.sh
# ==============================================================================
set -e

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BINARY_FILE="$ROOT_DIR/dist/comfyui-desktop"
DOWNLOADER_FILE="$ROOT_DIR/dist/comfyui-downloader"
LOCAL_DIR="$HOME/.local"

echo "========================================"
echo " ComfyUI Desktop — Local Installer"
echo "========================================"
echo ""

if [ -f "$BINARY_FILE" ]; then
    echo "⚙️ Found compiled binary: $BINARY_FILE"
else
    echo "❌ Error: Compiled binary not found in $ROOT_DIR/dist/"
    echo "Please run the build script first:"
    echo "  bash scripts/build.sh"
    exit 1
fi

echo "[1/4] Preparing installation directories..."
mkdir -p "$LOCAL_DIR/bin"
mkdir -p "$LOCAL_DIR/share/applications"

echo "[2/4] Installing binary..."
cp "$BINARY_FILE" "$LOCAL_DIR/bin/comfyui-desktop"
chmod +x "$LOCAL_DIR/bin/comfyui-desktop"
echo "  ✅ Binary installed to: $LOCAL_DIR/bin/comfyui-desktop"

if [ -f "$DOWNLOADER_FILE" ]; then
    cp "$DOWNLOADER_FILE" "$LOCAL_DIR/bin/comfyui-downloader"
    chmod +x "$LOCAL_DIR/bin/comfyui-downloader"
    echo "  ✅ Downloader binary installed to: $LOCAL_DIR/bin/comfyui-downloader"
else
    echo "  ⚠️  Downloader binary not found in dist/. Skipping optional downloader installation."
fi

echo "[3/4] Installing application icons and generating desktop entry..."

# Install application icons from source assets
ICON_COUNT=0
ICONS_SRC_DIR="$ROOT_DIR/crates/launcher/assets/icons"

declare -A ICON_MAP=(
    ["32x32.png"]="32x32"
    ["64x64.png"]="64x64"
    ["128x128.png"]="128x128"
    ["128x128@2x.png"]="256x256"
    ["icon.png"]="512x512"
)

for icon_file in "${!ICON_MAP[@]}"; do
    size="${ICON_MAP[$icon_file]}"
    src_path="$ICONS_SRC_DIR/$icon_file"
    if [ -f "$src_path" ]; then
        dest_dir="$LOCAL_DIR/share/icons/hicolor/$size/apps"
        mkdir -p "$dest_dir"
        cp "$src_path" "$dest_dir/comfyui-desktop.png"
        echo "    ✅ Icon $size installed"
        ICON_COUNT=$((ICON_COUNT + 1))
    fi
done

if [ $ICON_COUNT -eq 0 ]; then
    echo "    ⚠️  No source icons found"
fi

# Generate desktop entry file
VERSION=$(grep -m1 '^version' "$ROOT_DIR/crates/launcher/Cargo.toml" | sed 's/.*"\(.*\)".*/\1/' || echo "0.5.0")
DEST_DESKTOP="$LOCAL_DIR/share/applications/comfyui-desktop.desktop"
cat > "$DEST_DESKTOP" <<EOF
[Desktop Entry]
Categories=Utility;
Comment=ComfyUI Desktop — Tauri v2 wrapper for ComfyUI with AMD ROCm GPU auto-configuration
Exec=$LOCAL_DIR/bin/comfyui-desktop
Icon=comfyui-desktop
Name=ComfyUI Desktop
Terminal=false
Type=Application
Version=$VERSION
StartupWMClass=comfyui-desktop
EOF
echo "  ✅ Desktop entry generated and installed to: $DEST_DESKTOP"

echo "[4/4] Updating desktop database..."

# Update system databases to immediately detect the new shortcut and icons
update-desktop-database "$LOCAL_DIR/share/applications" 2>/dev/null || true
gtk-update-icon-cache -f -t "$LOCAL_DIR/share/icons/hicolor" 2>/dev/null || true

echo ""
echo "========================================"
echo "✅ COMPLETE! ComfyUI Desktop is installed."
echo "Please search for 'ComfyUI Desktop' in your application menu"
echo "or run the following command in terminal:"
echo "  comfyui-desktop"
echo "========================================"
