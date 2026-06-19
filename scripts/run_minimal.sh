#!/usr/bin/env bash
set -euo pipefail

ROOT="/home/raka/App/ComfyUi-arwaky"
PY="$ROOT/venv/bin/python3.12"
MAIN="$ROOT/ComfyUI/main.py"
PORT=8188

echo "[minimal] kill port $PORT ..."
fuser -k "${PORT}/tcp" 2>/dev/null || true
sleep 1

# ROCm 7.2.4 + gfx1030 stability fixes
export PATH="/opt/rocm-7.2.4/bin:$PATH"
export HIP_VISIBLE_DEVICES="0"
export HSA_OVERRIDE_GFX_VERSION="10.3.0"
COMFYUI_XDG_CACHE="${HOME}/.cache/comfyui-desktop"
COMFYUI_HIP_CACHE="${COMFYUI_XDG_CACHE}/hip"
mkdir -p "$COMFYUI_XDG_CACHE" "$COMFYUI_HIP_CACHE"
export XDG_CACHE_HOME="$COMFYUI_XDG_CACHE"
export HIP_CACHE_DIR="$COMFYUI_HIP_CACHE"

# Fix JIT compile deadlock + memory allocator hang
export PYTORCH_HIP_ALLOC_CONF="expandable_segments:1"
export AMD_SERIALIZE_KERNEL=3
export HIP_LAUNCH_QUEUE_DUPLICATE_KERNEL_ATTENUATION=1
export TORCH_HIP_DISABLE_TRITON_MMA=1
export HIP_DISABLE_RDC=1
export HIPCC_VERBOSE=0

echo "[minimal] starting comfyui with ROCm fixes ..."
exec "$PY" "$MAIN" \
  --listen 127.0.0.1 \
  --port "$PORT" \
  --highvram
