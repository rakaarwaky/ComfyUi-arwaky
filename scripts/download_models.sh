#!/bin/bash
# Launcher for ComfyUI Model Downloader (Rust TUI)
# This script builds and launches the Rust downloader from crates/downloader

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
DOWNLOADER_DIR="$PROJECT_ROOT/crates/downloader"
BINARY="$DOWNLOADER_DIR/target/release/comfyui-downloader"

RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

echo -e "${CYAN}${BOLD}ComfyUI Model Downloader${NC}"
echo ""

# Build if binary doesn't exist or source is newer
NEED_BUILD=false
if [ ! -f "$BINARY" ]; then
    echo -e "${CYAN}Binary not found. Building...${NC}"
    NEED_BUILD=true
else
    # Check if any source file is newer than the binary
    BINARY_TIME=$(stat -c %Y "$BINARY" 2>/dev/null || echo 0)
    for src in "$DOWNLOADER_DIR"/src/*.rs "$DOWNLOADER_DIR"/Cargo.toml "$DOWNLOADER_DIR"/models.json; do
        if [ -f "$src" ]; then
            SRC_TIME=$(stat -c %Y "$src" 2>/dev/null || echo 0)
            if [ "$SRC_TIME" -gt "$BINARY_TIME" ]; then
                echo -e "${CYAN}Source changes detected. Rebuilding...${NC}"
                NEED_BUILD=true
                break
            fi
        fi
    done
fi

if [ "$NEED_BUILD" = true ]; then
    echo -e "${CYAN}Building comfyui-downloader...${NC}"
    if ! cargo build --release --manifest-path "$DOWNLOADER_DIR/Cargo.toml" 2>&1; then
        echo -e "${RED}Build failed. Install Rust from https://rustup.rs${NC}"
        exit 1
    fi
    echo -e "${GREEN}Build successful.${NC}"
    echo ""
fi

# Launch the Rust binary with all passed arguments
exec "$BINARY" "$@"
