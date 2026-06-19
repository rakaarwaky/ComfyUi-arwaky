#!/usr/bin/env bash
# ==============================================================================
# install_deps.sh — Full setup: Python venv, deps, ComfyUI-Manager, Frontend
# Run from root project: bash scripts/install_deps.sh
# ==============================================================================
set -e

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VENV_DIR="$ROOT_DIR/venv"
REQ_FILE="$ROOT_DIR/requirements.txt"

echo "========================================"
echo " ComfyUI Desktop — Full Installer"
echo "========================================"
echo ""

# 1. Check if python3.12 is installed
echo "[1/5] Checking for Python 3.12..."
if ! command -v python3.12 &>/dev/null; then
    echo "❌ Error: Python 3.12 is not installed on this system."
    echo "This project requires Python 3.12 specifically for ROCm compatibility."
    echo "Please install it using your package manager, for example:"
    echo "  sudo dnf install -y python3.12 python3.12-devel"
    exit 1
fi
echo "  ✅ Python 3.12 is available: $(python3.12 --version)"

# 2. Check and initialize virtual environment
echo "[2/5] Setting up Python virtual environment..."
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
echo "[3/5] Installing Python dependencies from requirements.txt..."
if [ ! -f "$REQ_FILE" ]; then
    echo "❌ Error: requirements.txt not found at $REQ_FILE"
    exit 1
fi

echo "  This may take a few minutes as it downloads PyTorch ROCm wheels..."
pip install -r "$REQ_FILE"

echo ""
echo "✅ Python dependencies installed successfully."

# 4. Setup ComfyUI-Manager (forked version)
echo ""
echo "[4/5] Setting up ComfyUI-Manager..."
CUSTOM_NODES_DIR="$ROOT_DIR/ComfyUI/custom_nodes"
FORKED_MANAGER="$ROOT_DIR/ComfyUI-Manager"
MANAGER_CONFIG="$ROOT_DIR/ComfyUI/user/__manager/config.ini"

if [ -d "$FORKED_MANAGER" ]; then
    if [ -d "$CUSTOM_NODES_DIR/ComfyUI-Manager" ]; then
        rm -rf "$CUSTOM_NODES_DIR/ComfyUI-Manager"
    fi
    cp -r "$FORKED_MANAGER" "$CUSTOM_NODES_DIR/ComfyUI-Manager"
    echo "  ✅ Using forked ComfyUI-Manager from $FORKED_MANAGER"
else
    echo "  ⚠️  Forked ComfyUI-Manager not found at $FORKED_MANAGER"
    echo "     Using existing ComfyUI-Manager in custom_nodes/"
fi

# Disable network fetch in ComfyUI-Manager
if [ -f "$MANAGER_CONFIG" ]; then
    sed -i 's/^network_mode = .*/network_mode = offline/' "$MANAGER_CONFIG"
    echo "  ✅ ComfyUI-Manager network_mode set to offline"
else
    mkdir -p "$(dirname "$MANAGER_CONFIG")"
    echo -e "[default]\nnetwork_mode = offline" > "$MANAGER_CONFIG"
    echo "  ✅ ComfyUI-Manager config created with network_mode=offline"
fi

echo ""
echo "✅ ComfyUI-Manager setup complete."

# 5. Build ComfyUI Frontend (forked version)
echo ""
echo "[5/5] Building ComfyUI Frontend..."
FRONTEND_DIR="$ROOT_DIR/ComfyUI_frontend"

if [ -d "$FRONTEND_DIR" ]; then
    echo "  Installing frontend dependencies..."
    cd "$FRONTEND_DIR"
    
    # Check if pnpm is available
    if command -v pnpm &>/dev/null; then
        pnpm install --frozen-lockfile 2>/dev/null || pnpm install --ignore-engines
        echo "  Building frontend..."
        pnpm build
        echo "  ✅ Frontend built successfully."
    elif command -v npm &>/dev/null; then
        echo "  ⚠️  pnpm not found, using npm instead."
        npm install
        npm run build
        echo "  ✅ Frontend built successfully."
    else
        echo "  ❌ Error: Neither pnpm nor npm found."
        echo "     Please install Node.js and pnpm/npm."
        exit 1
    fi
    cd "$ROOT_DIR"
else
    echo "  ⚠️  ComfyUI_frontend not found at $FRONTEND_DIR"
    echo "     Skipping frontend build."
fi

echo ""
echo "========================================"
echo "✅ FULL SETUP COMPLETE!"
echo "========================================"
echo ""
echo "Next steps:"
echo "  1. Run: bash scripts/run_comfyui.sh"
echo "  2. Or use menu: ./comfyui-arwaky"
