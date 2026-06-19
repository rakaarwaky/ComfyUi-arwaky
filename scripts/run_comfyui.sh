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
#   --vram-mode MODE  VRAM mode: auto|high|normal|low|cpu (default: auto)
#   -h, --help        Show this help
#
# Examples:
#   bash scripts/run_comfyui.sh
#   bash scripts/run_comfyui.sh --port 8288
#   bash scripts/run_comfyui.sh --no-guard -- --force-fp16
#   bash scripts/run_comfyui.sh --vram-mode high
# ==============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"

# Ensure ROCm tools (like rocm-smi) are in PATH even when launched from GUI environments (Tauri/Desktop)
export PATH="/opt/rocm/bin:/opt/rocm-7.2.4/bin:$PATH"

PORT=8188
GUARD=true
GPU_DETECT=true
VRAM_MODE=""

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
    --vram-mode)
      if [[ $# -lt 2 ]]; then
        echo "Error: --vram-mode requires a value (auto|high|normal|low|cpu)" >&2
        exit 1
      fi
      VRAM_MODE="$2"
      shift 2
      ;;
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

echo "ComfyUI launcher — GPU $DETECTED_GPU | port $PORT | guard=$GUARD | vram_mode=${VRAM_MODE:-auto}"

export HIP_VISIBLE_DEVICES="$DETECTED_GPU"
export HSA_OVERRIDE_GFX_VERSION="10.3.0"

COMFYUI_VRAM_ARGS=()
if [[ "$VRAM_MODE" == "auto" || -z "$VRAM_MODE" ]]; then
  VRAM_BYTES=$(grep "GPU\[${DETECTED_GPU}\]" -A 20 <(rocm-smi --showmeminfo vram 2>/dev/null) \
    | grep "VRAM Total Memory" | head -n1 | grep -oE '[0-9]{5,}' | head -n1 || true)
  VRAM_BYTES="${VRAM_BYTES:-0}"
  if [[ "$VRAM_BYTES" -ge 12000000000 ]]; then
    COMFYUI_VRAM_ARGS=(--highvram)
  elif [[ "$VRAM_BYTES" -ge 6000000000 ]]; then
    COMFYUI_VRAM_ARGS=(--normalvram)
  else
    COMFYUI_VRAM_ARGS=(--lowvram)
  fi
elif [[ "$VRAM_MODE" == "high" ]]; then
  COMFYUI_VRAM_ARGS=(--highvram)
elif [[ "$VRAM_MODE" == "normal" ]]; then
  COMFYUI_VRAM_ARGS=(--normalvram)
elif [[ "$VRAM_MODE" == "low" ]]; then
  COMFYUI_VRAM_ARGS=(--lowvram)
elif [[ "$VRAM_MODE" == "cpu" ]]; then
  COMFYUI_VRAM_ARGS=(--cpu)
else
  echo "Warning: unknown --vram-mode '$VRAM_MODE', falling back to auto" >&2
  COMFYUI_VRAM_ARGS=(--normalvram)
fi

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
  "${COMFYUI_VRAM_ARGS[@]}" \
  "${EXTRA_ARGS[@]}"
