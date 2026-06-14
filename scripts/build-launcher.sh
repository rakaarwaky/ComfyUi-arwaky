#!/usr/bin/env bash
# ==============================================================================
# build-launcher.sh — Build ComfyUI Desktop Launcher (Tauri v2) release binary
# Run from root project: bash scripts/build-launcher.sh
# ==============================================================================
set -e

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST_DIR="$ROOT_DIR/dist"

echo "========================================"
echo " ComfyUI Desktop Launcher — Tauri Build"
echo "========================================"
echo ""

echo "[1/2] Building Tauri app (cargo only)..."
cd "$ROOT_DIR"
export NO_STRIP=true
npx @tauri-apps/cli@latest build --no-bundle

echo ""
echo "[2/2] Copying build artifact to dist/..."

mkdir -p "$DIST_DIR"

if [ -f "$ROOT_DIR/crates/launcher/target/release/app" ]; then
    cp "$ROOT_DIR/crates/launcher/target/release/app" "$DIST_DIR/comfyui-desktop"
    echo "  ✅ Binary copied to dist/comfyui-desktop"
else
    echo "  ❌ Build artifact not found at crates/launcher/target/release/app"
    exit 1
fi

echo ""
echo "========================================"
echo "✅ COMPLETE! dist/comfyui-desktop"
echo "========================================"
