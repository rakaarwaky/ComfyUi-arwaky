#!/usr/bin/env bash
# ==============================================================================
# build-downloader.sh — Build ComfyUI Model Downloader release binary
# Run from root project: bash scripts/build-downloader.sh
# ==============================================================================
set -e

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST_DIR="$ROOT_DIR/dist"

echo "========================================"
echo " ComfyUI Model Downloader — Cargo Build"
echo "========================================"
echo ""

echo "[1/2] Building downloader (cargo release)..."
cargo build --release --manifest-path "$ROOT_DIR/crates/downloader/Cargo.toml"

echo ""
echo "[2/2] Copying build artifact to dist/..."

mkdir -p "$DIST_DIR"

if [ -f "$ROOT_DIR/crates/downloader/target/release/comfyui-downloader" ]; then
    cp "$ROOT_DIR/crates/downloader/target/release/comfyui-downloader" "$DIST_DIR/comfyui-downloader"
    echo "  ✅ Binary copied to dist/comfyui-downloader"
else
    echo "  ❌ Build artifact not found at crates/downloader/target/release/comfyui-downloader"
    exit 1
fi

echo ""
echo "========================================"
echo "✅ COMPLETE! dist/comfyui-downloader"
echo "========================================"
