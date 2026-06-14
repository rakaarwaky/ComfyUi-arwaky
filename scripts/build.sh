#!/usr/bin/env bash
# ==============================================================================
# build.sh — Build ComfyUI Desktop (Tauri v2) and output to dist/
# Run from root project: bash scripts/build.sh
# ==============================================================================
set -e

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST_DIR="$ROOT_DIR/dist"
BUNDLE_DIR="$ROOT_DIR/crates/launcher/target/release/bundle"

echo "========================================"
# ComfyUI Desktop — Tauri Build
echo " ComfyUI Desktop — Tauri Build"
echo "========================================"
echo ""

# --- 1. Build Tauri ---
echo "[1/3] Building Tauri app..."
cd "$ROOT_DIR"
export NO_STRIP=true
npx @tauri-apps/cli@latest build

echo ""
echo "[2/3] Copying build artifacts to dist/..."

# Clean and recreate dist/
rm -rf "$DIST_DIR"
mkdir -p "$DIST_DIR"

# Copy RPM (Fedora installer)
if ls "$BUNDLE_DIR/rpm/"*.rpm 2>/dev/null; then
    cp "$BUNDLE_DIR/rpm/"*.rpm "$DIST_DIR/"
    echo "  ✅ RPM copied to dist/"
fi


# Copy standard binary
if [ -f "$ROOT_DIR/crates/launcher/target/release/app" ]; then
    cp "$ROOT_DIR/crates/launcher/target/release/app" "$DIST_DIR/comfyui-desktop"
    echo "  ✅ Binary copied to dist/comfyui-desktop"
fi

# Generate checksums for all artifacts
echo ""
echo "  Generating SHA256 checksums..."
cd "$DIST_DIR"
for file in *.rpm comfyui-desktop; do
    if [ -f "$file" ]; then
        sha256sum "$file" >> SHA256SUMS.txt
        echo "    ✅ $file checksum generated"
    fi
done
cd "$ROOT_DIR"

echo ""
echo "[3/3] Build results:"
ls -lh "$DIST_DIR/"

echo ""
echo "========================================"
echo "✅ COMPLETE! Files available in: dist/"
echo "========================================"
