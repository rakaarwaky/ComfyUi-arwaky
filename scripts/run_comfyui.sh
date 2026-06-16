#!/usr/bin/env bash
# ==============================================================================
# run_comfyui.sh — ComfyUI Server launcher with GPU detection, port guard, venv
#
# Usage:
#   bash scripts/run_comfyui.sh [options] [-- extra python args]
#
# Options:
#   --port PORT       Port to listen on (default: 8188)
#   --no-guard        Skip port-in-use check
#   --no-gpu-detect   Skip GPU detection, use device 0
#   -h, --help        Show this help
#
# Examples:
#   bash scripts/run_comfyui.sh
#   bash scripts/run_comfyui.sh --port 8288
#   bash scripts/run_comfyui.sh --no-guard -- --force-fp16
# ==============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"

PORT=8188
GUARD=true
GPU_DETECT=true

usage() {
  sed -n '3,17p' "$0" | sed 's/^#//; s/^ //'
  exit 0
}

# Parse known options, pass rest to ComfyUI
EXTRA_ARGS=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    -h|--help) usage ;;
    --port) PORT="$2"; shift 2 ;;
    --no-guard) GUARD=false; shift ;;
    --no-gpu-detect) GPU_DETECT=false; shift ;;
    --) shift; EXTRA_ARGS+=("$@"); break ;;
    *) EXTRA_ARGS+=("$1"); shift ;;
  esac
done

# ── Port guard ─────────────────────────────────────────────────────────────────
if $GUARD; then
  if ss -tlnp 2>/dev/null | grep -q ":${PORT} "; then
    echo "ComfyUI already running on port ${PORT}, skipping start."
    # Wait forever so systemd/service manager thinks process is healthy
    sleep infinity
  fi
fi

# ── GPU detection ──────────────────────────────────────────────────────────────
DETECTED_GPU="0"
if $GPU_DETECT; then
  DETECTED_GPU=$(rocm-smi --showmeminfo vram 2>/dev/null \
    | grep "VRAM Total Memory" \
    | awk -F'[][]' '{print $2, $NF}' \
    | awk '{print $1, $NF}' \
    | sort -k2 -rn \
    | head -n 1 \
    | awk '{print $1}')
  DETECTED_GPU="${DETECTED_GPU:-0}"
fi

echo "ComfyUI launcher — GPU $DETECTED_GPU | port $PORT | guard=$GUARD"

export HIP_VISIBLE_DEVICES="$DETECTED_GPU"
export HSA_OVERRIDE_GFX_VERSION="10.3.0"

# ── Activate venv ──────────────────────────────────────────────────────────────
if [ -f "$ROOT_DIR/venv/bin/activate" ]; then
  # shellcheck source=/dev/null
  source "$ROOT_DIR/venv/bin/activate"
else
  echo "Warning: venv not found at $ROOT_DIR/venv — skipping activation"
fi

# ── Launch ─────────────────────────────────────────────────────────────────────
echo "Starting ComfyUI Server on port $PORT..."
exec python "$ROOT_DIR/ComfyUI/main.py" \
  --extra-model-paths-config "$ROOT_DIR/extra_model_paths.yaml" \
  --output-directory "$HOME/SharedData/Output" \
  --input-directory "$HOME/SharedData/Input" \
  --user-directory "$HOME/SharedData/User" \
  --listen 127.0.0.1 \
  --port "$PORT" \
  "${EXTRA_ARGS[@]}"
