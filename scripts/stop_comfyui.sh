#!/usr/bin/env bash
# ==============================================================================
# stop_comfyui.sh — Stop ComfyUI desktop app + backend + free port 8188
# ==============================================================================
set -euo pipefail

PORT="${1:-8188}"
STOPPED=0

echo "Stopping ComfyUI (desktop + backend)..."

# 1. Kill comfyui-desktop (Tauri app)
if pgrep -x comfyui-desktop >/dev/null 2>&1; then
    pkill -x comfyui-desktop 2>/dev/null || true
    echo "  ✅ comfyui-desktop stopped"
    STOPPED=1
else
    echo "  ℹ️  comfyui-desktop not running"
fi

# 2. Kill python main.py (backend spawned by launcher)
if pgrep -f 'python.*main\.py' >/dev/null 2>&1; then
    pkill -f 'python.*main\.py' 2>/dev/null || true
    echo "  ✅ python main.py (backend) stopped"
    STOPPED=1
else
    echo "  ℹ️  python main.py not running"
fi

# 3. Kill anything on port 8188
if ss -tlnp 2>/dev/null | grep -q ":${PORT} "; then
    fuser -k "${PORT}/tcp" 2>/dev/null || true
    echo "  ✅ port $PORT freed"
    STOPPED=1
else
    echo "  ℹ️  port $PORT already free"
fi

if [ "$STOPPED" -eq 1 ]; then
    sleep 1
    echo "Done. All ComfyUI processes stopped."
else
    echo "Nothing to stop — ComfyUI is not running."
fi
