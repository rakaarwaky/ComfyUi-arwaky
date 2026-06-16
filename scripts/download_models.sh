#!/bin/bash
# Launcher for ComfyUI Model Downloader TUI (Rust TUI)
# This script builds and launches the Rust downloader TUI from crates/

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
BINARY="$PROJECT_ROOT/crates/target/release/comfyui-downloader-tui"

RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

echo -e "${CYAN}${BOLD}ComfyUI Model Downloader (TUI)${NC}"
echo ""

# Build if binary doesn't exist or source is newer
NEED_BUILD=false
if [ ! -f "$BINARY" ]; then
    echo -e "${CYAN}Binary not found. Building...${NC}"
    NEED_BUILD=true
else
    # Check if any source file is newer than the binary
    BINARY_TIME=$(stat -c %Y "$BINARY" 2>/dev/null || echo 0)
    # Walk all crate source files in crates/downloader/ (multi-crate structure)
    for src in "$PROJECT_ROOT"/crates/downloader/{*,*/*}/src/*.rs "$PROJECT_ROOT"/crates/downloader/{*,*/*}/Cargo.toml "$PROJECT_ROOT"/crates/downloader/config/models.json; do
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
    echo -e "${CYAN}Building comfyui-downloader-tui...${NC}"
    if ! cargo build --release --manifest-path "$PROJECT_ROOT/crates/Cargo.toml" -p downloader --bin comfyui-downloader-tui 2>&1; then
        echo -e "${RED}Build failed. Install Rust from https://rustup.rs${NC}"
        exit 1
    fi
    echo -e "${GREEN}Build successful.${NC}"
    echo ""
fi

# Launch the Rust TUI binary with all passed arguments
exec "$BINARY" "$@"
