#!/usr/bin/env bash
# ==============================================================================
# install_local.sh — Extract RPM and install locally to ~/.local/
# Run from root project: bash scripts/install_local.sh
# ==============================================================================
set -e

# ==============================================================================
# install_local.sh — Extract RPM or install binary locally to ~/.local/
# Run from root project: bash scripts/install_local.sh
# ==============================================================================
set -e

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck disable=SC2012
RPM_FILE=$(ls -t "$ROOT_DIR/dist"/comfyui-desktop-*.rpm 2>/dev/null | head -n 1)
BINARY_FILE="$ROOT_DIR/dist/comfyui-desktop"
LOCAL_DIR="$HOME/.local"
TEMP_EXTRACT_DIR="/tmp/comfyui_desktop_rpm_extract"

echo "========================================"
echo " ComfyUI Desktop — Local Installer"
echo "========================================"
echo ""

if [ -n "$RPM_FILE" ] && [ -f "$RPM_FILE" ]; then
    echo "📦 Found RPM package: $RPM_FILE"
    INSTALL_MODE="rpm"
elif [ -f "$BINARY_FILE" ]; then
    echo "⚙️ Found compiled binary: $BINARY_FILE"
    INSTALL_MODE="binary"
else
    echo "❌ Error: Neither RPM package nor compiled binary found in $ROOT_DIR/dist/"
    echo "Please run the build script first:"
    echo "  bash scripts/build.sh"
    exit 1
fi

if [ "$INSTALL_MODE" = "rpm" ]; then
    echo "[1/4] Preparing extraction directory..."
    rm -rf "$TEMP_EXTRACT_DIR"
    mkdir -p "$TEMP_EXTRACT_DIR"

    echo "[2/4] Extracting files from RPM..."
    cd "$TEMP_EXTRACT_DIR"
    rpm2cpio "$RPM_FILE" | cpio -idmv > /dev/null 2>&1

    echo "[3/4] Installing to $LOCAL_DIR..."

    # 1. Install binary
    mkdir -p "$LOCAL_DIR/bin"
    rm -f "$LOCAL_DIR/bin/comfyui-desktop"
    if [ -f "./usr/bin/comfyui-desktop" ]; then
        cp "./usr/bin/comfyui-desktop" "$LOCAL_DIR/bin/comfyui-desktop"
        chmod +x "$LOCAL_DIR/bin/comfyui-desktop"
        echo "  ✅ Binary installed to: $LOCAL_DIR/bin/comfyui-desktop"
    elif [ -f "./usr/bin/app" ]; then
        cp "./usr/bin/app" "$LOCAL_DIR/bin/comfyui-desktop"
        chmod +x "$LOCAL_DIR/bin/comfyui-desktop"
        echo "  ✅ Binary installed to: $LOCAL_DIR/bin/comfyui-desktop"
    else
        echo "❌ Error: Binary not found inside RPM (searched for comfyui-desktop and app)"
        exit 1
    fi

    # 2. Install application icons (auto-detect all sizes)
    echo "  Installing icons..."
    ICON_COUNT=0
    while IFS= read -r -d '' SRC_ICON; do
        SIZE=$(echo "$SRC_ICON" | sed -n 's|.*/hicolor/\([^/]*\)/apps/.*|\1|p')
        if [ -z "$SIZE" ]; then
            continue
        fi
        DEST_ICON_DIR="$LOCAL_DIR/share/icons/hicolor/$SIZE/apps"
        mkdir -p "$DEST_ICON_DIR"
        cp "$SRC_ICON" "$DEST_ICON_DIR/comfyui-desktop.png"
        echo "    ✅ Icon $SIZE installed"
        ICON_COUNT=$((ICON_COUNT + 1))
    done < <(find ./usr/share/icons/hicolor \( -name "comfyui-desktop.png" -o -name "app.png" -o -name "icon.png" \) -type f -print0 2>/dev/null)
    if [ $ICON_COUNT -eq 0 ]; then
        echo "    ⚠️  No icons found in RPM (this may be normal if icons aren't bundled)"
    fi

    # 3. Install desktop entry file
    mkdir -p "$LOCAL_DIR/share/applications"
    SRC_DESKTOP="./usr/share/applications/comfyui-desktop.desktop"
    DEST_DESKTOP="$LOCAL_DIR/share/applications/comfyui-desktop.desktop"

    if [ -f "$SRC_DESKTOP" ]; then
        cp "$SRC_DESKTOP" "$DEST_DESKTOP"
        
        # Update Exec, Icon, and StartupWMClass in desktop entry for local installation
        sed -i "s|^Exec=.*|Exec=$LOCAL_DIR/bin/comfyui-desktop|g" "$DEST_DESKTOP"
        sed -i "s|^Icon=.*|Icon=comfyui-desktop|g" "$DEST_DESKTOP"
        sed -i "s|^StartupWMClass=.*|StartupWMClass=comfyui-desktop|g" "$DEST_DESKTOP"
        
        echo "  ✅ Desktop entry installed to: $DEST_DESKTOP"
    else
        echo "❌ Error: Desktop entry not found inside RPM"
        exit 1
    fi

    echo "[4/4] Updating desktop database..."
    # Clean up temp extraction dir
    rm -rf "$TEMP_EXTRACT_DIR"

else
    echo "[1/4] Preparing installation directories..."
    mkdir -p "$LOCAL_DIR/bin"
    mkdir -p "$LOCAL_DIR/share/applications"

    echo "[2/4] Installing binary..."
    cp "$BINARY_FILE" "$LOCAL_DIR/bin/comfyui-desktop"
    chmod +x "$LOCAL_DIR/bin/comfyui-desktop"
    echo "  ✅ Binary installed to: $LOCAL_DIR/bin/comfyui-desktop"

    if [ -f "$ROOT_DIR/dist/comfyui-downloader" ]; then
        cp "$ROOT_DIR/dist/comfyui-downloader" "$LOCAL_DIR/bin/comfyui-downloader"
        chmod +x "$LOCAL_DIR/bin/comfyui-downloader"
        echo "  ✅ Downloader binary installed to: $LOCAL_DIR/bin/comfyui-downloader"
    fi

    echo "[3/4] Installing application icons and generating desktop entry..."
    
    # 2. Install application icons from source assets
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

    # 3. Generate desktop entry file
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
fi

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
