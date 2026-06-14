#!/usr/bin/env bash
# ==============================================================================
# build.sh — Build ComfyUI Desktop (Tauri v2) and output to dist/ (Cargo only, no RPM)
# Run from root project: bash scripts/build.sh
# ==============================================================================
set -e

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST_DIR="$ROOT_DIR/dist"

echo "========================================"
# ComfyUI Desktop — Tauri Build (Cargo Only)
echo " ComfyUI Desktop — Tauri Build (Cargo Only)"
echo "========================================"
echo ""

# --- 1. Build Tauri ---
echo "[1/3] Building Tauri app (cargo only)..."
cd "$ROOT_DIR"
export NO_STRIP=true
npx @tauri-apps/cli@latest build --no-bundle

echo ""
echo "Building comfyui-downloader (cargo release)..."
cargo build --release --manifest-path "$ROOT_DIR/crates/downloader/Cargo.toml"

echo ""
echo "[2/3] Copying build artifacts to dist/..."

# Clean and recreate dist/
rm -rf "$DIST_DIR"
mkdir -p "$DIST_DIR"

# Copy standard binary
if [ -f "$ROOT_DIR/crates/launcher/target/release/app" ]; then
    cp "$ROOT_DIR/crates/launcher/target/release/app" "$DIST_DIR/comfyui-desktop"
    echo "  ✅ Binary copied to dist/comfyui-desktop"
fi

# Copy downloader binary
if [ -f "$ROOT_DIR/crates/downloader/target/release/comfyui-downloader" ]; then
    cp "$ROOT_DIR/crates/downloader/target/release/comfyui-downloader" "$DIST_DIR/comfyui-downloader"
    echo "  ✅ Downloader binary copied to dist/comfyui-downloader"
fi

# Generate checksums for all artifacts
echo ""
echo "  Generating SHA256 checksums..."
cd "$DIST_DIR"
if [ -f "comfyui-desktop" ]; then
    sha256sum "comfyui-desktop" >> SHA256SUMS.txt
    echo "    ✅ comfyui-desktop checksum generated"
fi
if [ -f "comfyui-downloader" ]; then
    sha256sum "comfyui-downloader" >> SHA256SUMS.txt
    echo "    ✅ comfyui-downloader checksum generated"
fi
cd "$ROOT_DIR"

echo ""
echo "[3/3] Build results:"
ls -lh "$DIST_DIR/"

echo ""
echo "========================================"
echo "✅ COMPLETE! Files available in: dist/"
echo "========================================"
