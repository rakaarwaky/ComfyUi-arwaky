#!/bin/bash
# Script to download required models for cartoon_to_realistic_sdxl_api2_shot_01 workflow

CHECKPOINT_DIR="$HOME/SharedData/Models/checkpoints"
CONTROLNET_DIR="$HOME/SharedData/Models/controlnet"

mkdir -p "$CHECKPOINT_DIR"
mkdir -p "$CONTROLNET_DIR"

echo "=== 1/4 Downloading Juggernaut XL Checkpoint (6.6 GB) ==="
wget -c --show-progress "https://huggingface.co/RunDiffusion/Juggernaut-XL-v9/resolve/main/Juggernaut-XL_v9_RunDiffusionPhoto_v2.safetensors" \
  -O "$CHECKPOINT_DIR/Juggernaut-XL_v9_RunDiffusionPhoto_v2.safetensors"

echo "=== 2/4 Downloading ControlNet Depth XL (2.5 GB) ==="
wget -c --show-progress "https://huggingface.co/lllyasviel/sd_control_collection/resolve/main/diffusers_xl_depth_mid.safetensors" \
  -O "$CONTROLNET_DIR/diffusers_xl_depth_mid.safetensors"

echo "=== 3/4 Downloading ControlNet Canny XL (2.5 GB) ==="
wget -c --show-progress "https://huggingface.co/lllyasviel/sd_control_collection/resolve/main/diffusers_xl_canny_mid.safetensors" \
  -O "$CONTROLNET_DIR/diffusers_xl_canny_mid.safetensors"

echo "=== 4/4 Downloading ControlNet Normal XL (2.5 GB) ==="
wget -c --show-progress "https://huggingface.co/Eugeoter/noob-sdxl-controlnet-normal/resolve/main/noob-sdxl-controlnet-normal.safetensors" \
  -O "$CONTROLNET_DIR/noob-sdxl-controlnet-normal.safetensors"

echo "=== All model downloads completed! ==="
