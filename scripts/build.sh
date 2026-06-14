#!/usr/bin/env bash
# ==============================================================================
# build.sh — Build ComfyUI Desktop (Launcher + Downloader) release binaries
# Run from root project: bash scripts/build.sh
# ==============================================================================
set -e

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST_DIR="$ROOT_DIR/dist"

echo "========================================"
echo " ComfyUI Desktop — Full Build"
echo "========================================"
echo ""

# Clean and recreate dist/
rm -rf "$DIST_DIR"

echo ">>> Building Launcher..."
bash "$ROOT_DIR/scripts/build-launcher.sh"

echo ""
echo ">>> Building Downloader..."
bash "$ROOT_DIR/scripts/build-downloader.sh"

# Generate checksums
echo ""
echo "Generating SHA256 checksums..."
cd "$DIST_DIR"
sha256sum comfyui-desktop comfyui-downloader > SHA256SUMS.txt
echo "  ✅ SHA256SUMS.txt generated"
cd "$ROOT_DIR"

echo ""
echo "========================================"
echo "✅ COMPLETE! Files available in dist/:"
ls -lh "$DIST_DIR/"
echo "========================================"
