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

echo "[1/3] Building CLI binary (comfyui-downloader-cli)..."
cargo build --release --manifest-path "$ROOT_DIR/crates/Cargo.toml" -p downloader --bin comfyui-downloader-cli

echo ""
echo "[2/3] Building TUI binary (comfyui-downloader-tui)..."
cargo build --release --manifest-path "$ROOT_DIR/crates/Cargo.toml" -p downloader --bin comfyui-downloader-tui

echo ""
echo "[3/3] Copying build artifacts to dist/..."

mkdir -p "$DIST_DIR"

TARGET_DIR="$ROOT_DIR/crates/target/release"

if [ -f "$TARGET_DIR/comfyui-downloader-cli" ]; then
    cp "$TARGET_DIR/comfyui-downloader-cli" "$DIST_DIR/comfyui-downloader-cli"
    echo "  ✅ CLI binary -> dist/comfyui-downloader-cli"
else
    echo "  ❌ CLI binary not found at $TARGET_DIR/comfyui-downloader-cli"
    exit 1
fi

if [ -f "$TARGET_DIR/comfyui-downloader-tui" ]; then
    cp "$TARGET_DIR/comfyui-downloader-tui" "$DIST_DIR/comfyui-downloader-tui"
    echo "  ✅ TUI binary -> dist/comfyui-downloader-tui"
else
    echo "  ❌ TUI binary not found at $TARGET_DIR/comfyui-downloader-tui"
    exit 1
fi

echo ""
echo "========================================"
echo "✅ COMPLETE! Artifacts in dist/:"
ls -lh "$DIST_DIR/comfyui-downloader-"*
echo "========================================"
