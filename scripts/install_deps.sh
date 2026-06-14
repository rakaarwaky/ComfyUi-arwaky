#!/usr/bin/env bash
# ==============================================================================
# install_deps.sh — Install Python 3.12 Venv & pip requirements
# Run from root project: bash scripts/install_deps.sh
# ==============================================================================
set -e

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VENV_DIR="$ROOT_DIR/venv"
REQ_FILE="$ROOT_DIR/requirements.txt"

echo "========================================"
echo " ComfyUI Desktop — Python Venv & Pip Installer"
echo "========================================"
echo ""

# 1. Check if python3.12 is installed
echo "[1/3] Checking for Python 3.12..."
if ! command -v python3.12 &>/dev/null; then
    echo "❌ Error: Python 3.12 is not installed on this system."
    echo "This project requires Python 3.12 specifically for ROCm compatibility."
    echo "Please install it using your package manager, for example:"
    echo "  sudo dnf install -y python3.12 python3.12-devel"
    exit 1
fi
echo "  ✅ Python 3.12 is available: $(python3.12 --version)"

# 2. Check and initialize virtual environment
echo "[2/3] Setting up Python virtual environment..."
if [ ! -d "$VENV_DIR" ]; then
    echo "  Virtual environment not found. Creating one at $VENV_DIR..."
    python3.12 -m venv "$VENV_DIR"
    echo "  ✅ Virtual environment created."
else
    echo "  ✅ Virtual environment already exists at $VENV_DIR."
fi

# Activate venv for the remaining commands
# shellcheck source=/dev/null
source "$VENV_DIR/bin/activate"

# Upgrade pip and toolchain inside the venv
echo "  Upgrading pip, setuptools, and wheel..."
pip install --upgrade pip setuptools wheel

# 3. Install requirements.txt
echo "[3/3] Installing Python dependencies from requirements.txt..."
if [ ! -f "$REQ_FILE" ]; then
    echo "❌ Error: requirements.txt not found at $REQ_FILE"
    exit 1
fi

echo "  This may take a few minutes as it downloads PyTorch ROCm wheels..."
pip install -r "$REQ_FILE"

echo ""
echo "========================================"
echo "✅ COMPLETE! Python dependencies installed successfully."
echo "To activate this environment in your terminal, run:"
echo "  source venv/bin/activate"
echo "========================================"
