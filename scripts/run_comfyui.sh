#!/usr/bin/env bash
# ==============================================================================
# run_comfyui.sh — Run ComfyUI Server standalone (without wrapper)
# ==============================================================================

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"

# Auto-detect discrete GPU (dGPU) by comparing the largest VRAM sizes
DETECTED_GPU=$(rocm-smi --showmeminfo vram 2>/dev/null | grep "VRAM Total Memory" | awk -F'[][]' '{print $2, $NF}' | awk '{print $1, $NF}' | sort -k2 -rn | head -n 1 | awk '{print $1}')

if [ -z "$DETECTED_GPU" ]; then
  # Fallback default to GPU 0 if rocm-smi fails/is not present
  DETECTED_GPU="0"
fi

echo "Smart GPU Detection: Using GPU $DETECTED_GPU (discrete GPU with largest VRAM)"

export HIP_VISIBLE_DEVICES="$DETECTED_GPU"
export HSA_OVERRIDE_GFX_VERSION="10.3.0"

# Activate virtual environment
# shellcheck source=/dev/null
source "$ROOT_DIR/venv/bin/activate"

# Run ComfyUI with external model configuration
echo "Starting ComfyUI Server with external model configuration..."
python "$ROOT_DIR/ComfyUI/main.py" \
  --extra-model-paths-config "$ROOT_DIR/extra_model_paths.yaml" \
  --output-directory "$HOME/SharedData/Output" \
  --input-directory "$HOME/SharedData/Input" \
  --user-directory "$HOME/SharedData/User" \
  "$@"
