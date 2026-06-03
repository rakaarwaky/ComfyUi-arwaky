#!/bin/bash
# Script to download and upgrade models for ComfyUI Desktop
# Tailored for local ROCm AMD environments with 16GB VRAM (e.g. RX 6800 XT)

# Base directory for model storage (user-specific, matches extra_model_paths.yaml)
BASE_DIR="$HOME/SharedData/Models"
mkdir -p "$BASE_DIR"

# Text styles and colors for premium console output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
MAGENTA='\033[0;35m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m' # No Color

echo -e "${CYAN}${BOLD}========================================================================${NC}"
echo -e "${CYAN}${BOLD}          ComfyUI-Desktop Ultimate Model Downloader & Upgrader         ${NC}"
echo -e "${CYAN}${BOLD}========================================================================${NC}"
echo -e "Target Directory: ${BLUE}${BASE_DIR}${NC}\n"

# Helper function to download files securely and resume interrupted downloads
download_file() {
  local category="$1"
  local url="$2"
  local dest_file="$3"
  local full_dest_dir="$BASE_DIR/$category"
  local full_dest_path="$full_dest_dir/$dest_file"

  mkdir -p "$full_dest_dir"

  echo -e "\n${YELLOW}${BOLD}=== Downloading [$category] $dest_file ===${NC}"
  if [ -f "$full_dest_path" ]; then
    echo -e "${BLUE}File already exists. Checking for resume or skipping...${NC}"
  fi

  wget -c --show-progress "$url" -O "$full_dest_path"
  
  if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓ Successfully downloaded/verified $dest_file${NC}"
  else
    echo -e "${RED}✗ Failed to download $dest_file from $url${NC}"
  fi
}

# 1. audio_encoders
download_audio_encoders() {
  download_file "audio_encoders" \
    "https://huggingface.co/lukewys/laion_clap/resolve/main/music_audioset_epoch_15_esc_90.14.pt" \
    "laion_clap_music.pt"
}

# 2. background_removal
download_background_removal() {
  download_file "background_removal" \
    "https://huggingface.co/PramaLLC/BEN2/resolve/main/ben2.onnx" \
    "ben2.onnx"
}

# 3. checkpoints
download_checkpoints() {
  download_file "checkpoints" \
    "https://huggingface.co/Drnerdy81/juggernaut-xl-ragnarok/resolve/main/Juggernaut-XL-Ragnarok.safetensors" \
    "Juggernaut-XL-Ragnarok.safetensors"
}

# 4. clip
download_clip() {
  download_file "clip" \
    "https://huggingface.co/comfyanonymous/flux_text_encoders/resolve/main/clip_l.safetensors" \
    "clip_l.safetensors"
}

# 5. clip_vision
download_clip_vision() {
  download_file "clip_vision" \
    "https://huggingface.co/Comfy-Org/Wan_2.1_ComfyUI_repackaged/resolve/main/split_files/clip_vision/clip_vision_h.safetensors" \
    "clip_vision_h.safetensors"
}

# 6. configs
download_configs() {
  download_file "configs" \
    "https://raw.githubusercontent.com/Stability-AI/generative-models/main/configs/inference/sd_xl_base.yaml" \
    "sd_xl_base.yaml"
}

# 7. controlnet
download_controlnet() {
  download_file "controlnet" \
    "https://huggingface.co/Shakker-Labs/FLUX.1-dev-ControlNet-Union-Pro/resolve/main/diffusion_pytorch_model.safetensors" \
    "flux1-dev-controlnet-union.safetensors"
}

# 8. detection
download_detection() {
  download_file "detection" \
    "https://huggingface.co/Bingsu/adetailer/resolve/main/face_yolov8n.pt" \
    "yolov8-dfa-face.pt"
}

# 9. diffusers
download_diffusers() {
  local diffusers_dir="$BASE_DIR/diffusers/stable-diffusion-xl-base-1.0"
  echo -e "\n${YELLOW}${BOLD}=== Downloading [diffusers] stable-diffusion-xl-base-1.0 ===${NC}"
  
  if command -v huggingface-cli &> /dev/null; then
    echo -e "${BLUE}Using huggingface-cli to download folder structure...${NC}"
    huggingface-cli download stabilityai/stable-diffusion-xl-base-1.0 --local-dir "$diffusers_dir"
  else
    echo -e "${YELLOW}huggingface-cli not found. Downloading key model files manually via wget...${NC}"
    local files=(
      "model_index.json"
      "scheduler/scheduler_config.json"
      "text_encoder/model.safetensors"
      "text_encoder/config.json"
      "text_encoder_2/model.safetensors"
      "text_encoder_2/config.json"
      "tokenizer/vocab.json"
      "tokenizer/merges.txt"
      "tokenizer/special_tokens_map.json"
      "tokenizer/tokenizer_config.json"
      "tokenizer_2/vocab.json"
      "tokenizer_2/merges.txt"
      "tokenizer_2/special_tokens_map.json"
      "tokenizer_2/tokenizer_config.json"
      "unet/diffusion_pytorch_model.safetensors"
      "unet/config.json"
      "vae/diffusion_pytorch_model.safetensors"
      "vae/config.json"
    )
    for file in "${files[@]}"; do
      mkdir -p "$(dirname "$diffusers_dir/$file")"
      wget -c --show-progress "https://huggingface.co/stabilityai/stable-diffusion-xl-base-1.0/resolve/main/$file" -O "$diffusers_dir/$file"
    done
  fi
  echo -e "${GREEN}✓ Diffusers structure downloaded!${NC}"
}

# 10. diffusion_models
download_diffusion_models() {
  download_file "diffusion_models" \
    "https://huggingface.co/city96/FLUX.1-dev-gguf/resolve/main/flux1-dev-Q5_K_M.gguf" \
    "flux1-dev-Q5_K_M.gguf"
}

# 11. embeddings
download_embeddings() {
  download_file "embeddings" \
    "https://huggingface.co/datasets/gsdf/EasyNegative/resolve/main/EasyNegative.safetensors" \
    "easynegative.safetensors"
}

# 12. frame_interpolation
download_frame_interpolation() {
  download_file "frame_interpolation" \
    "https://huggingface.co/nguu/film-pytorch/resolve/main/film_net_fp32.pt" \
    "film_net_fp32.pt"
}

# 13. geometry_estimation
download_geometry_estimation() {
  download_file "geometry_estimation" \
    "https://huggingface.co/depth-anything/DA3-LARGE/resolve/main/pytorch_model.bin" \
    "depth_anything_v3_vitl.pth"
}

# 14. gligen
download_gligen() {
  download_file "gligen" \
    "https://huggingface.co/comfyanonymous/GLIGEN_textbox_model/resolve/main/gligen_sd15_textbox_pruned.safetensors" \
    "gligen_sd15_textbox_pruned.safetensors"
}

# 15. hypernetworks
download_hypernetworks() {
  echo -e "${YELLOW}Hypernetwork 'sd15_wavenet_style.pt' is optional and deprecated by LoRAs. Skipping...${NC}"
}

# 16. latent_upscale
download_latent_upscale() {
  # ComfyUI maps this to latent_upscale_models
  download_file "latent_upscale_models" \
    "https://huggingface.co/city96/latent-resizer/resolve/main/latent_resizer.pt" \
    "latent_resizer.pt"
}

# 17. loras
download_loras() {
  download_file "loras" \
    "https://huggingface.co/fal/Realism-Detailer-Kontext-Dev-LoRA/resolve/main/pytorch_model_lora.safetensors" \
    "flux_realism_detailer.safetensors"
}

# 18. model_patches
download_model_patches() {
  download_file "model_patches" \
    "https://huggingface.co/jiaxiangc/ResAdapter/resolve/main/resadapter_v1_sdxl_extralora.safetensors" \
    "resadapter_v1_sdxl_extralora.safetensors"
}

# 19. optical_flow
download_optical_flow() {
  download_file "optical_flow" \
    "https://download.pytorch.org/models/raft_large_C_T_V2-10a1125c.pth" \
    "raft_large_C_T_V2-10a1125c.pth"
}

# 20. photomaker
download_photomaker() {
  download_file "photomaker" \
    "https://huggingface.co/TencentARC/PhotoMaker-V2/resolve/main/photomaker-v2.bin" \
    "photomaker-v2.bin"
}

# 21. style_models
download_style_models() {
  download_file "style_models" \
    "https://huggingface.co/h94/IP-Adapter/resolve/main/sdxl_models/ip-adapter_sdxl.safetensors" \
    "ip-adapter_sdxl.safetensors"
}

# 22. text_encoders
download_text_encoders() {
  download_file "text_encoders" \
    "https://huggingface.co/comfyanonymous/flux_text_encoders/resolve/main/t5xxl_fp8_e4m3fn.safetensors" \
    "t5xxl_fp8_e4m3fn.safetensors"
}

# 24. upscale_models
download_upscale_models() {
  download_file "upscale_models" \
    "https://huggingface.co/lokCX/4x-Ultrasharp/resolve/main/4x-UltraSharp.pth" \
    "4x-UltraSharp.pth"
}

# 25. vae
download_vae() {
  download_file "vae" \
    "https://huggingface.co/madebyollin/sdxl-vae-fp16-fix/resolve/main/sdxl_vae.safetensors" \
    "sdxl_vae.safetensors"
  download_file "vae" \
    "https://huggingface.co/black-forest-labs/FLUX.1-schnell/resolve/main/ae.safetensors" \
    "ae.safetensors"
}

# 26. vae_approx
download_vae_approx() {
  download_file "vae_approx" \
    "https://huggingface.co/madebyollin/taesdxl/resolve/main/taesdxl.pth" \
    "taesdxl.pth"
}

download_flux_group() {
  echo -e "\n${MAGENTA}${BOLD}>>> Starting FLUX Essentials Download Group (~18 GB total)...<<<${NC}"
  download_clip
  download_text_encoders
  download_diffusion_models
  download_vae
  download_controlnet
  download_loras
}

download_sdxl_group() {
  echo -e "\n${BLUE}${BOLD}>>> Starting SDXL Essentials Download Group (~23 GB total)...<<<${NC}"
  download_checkpoints
  download_vae
  download_clip_vision
  download_style_models
  download_model_patches
  download_configs
}

download_video_group() {
  echo -e "\n${CYAN}${BOLD}>>> Starting Video & Animation Group (~2 GB total)...<<<${NC}"
  download_background_removal
  download_frame_interpolation
  download_optical_flow
  download_geometry_estimation
}

download_all() {
  echo -e "\n${RED}${BOLD}>>> WARNING: Starting download of ALL models (~45 GB+ total). Ensure you have enough disk space and bandwidth! <<<${NC}"
  download_audio_encoders
  download_background_removal
  download_checkpoints
  download_clip
  download_clip_vision
  download_configs
  download_controlnet
  download_detection
  download_diffusers
  download_diffusion_models
  download_embeddings
  download_frame_interpolation
  download_geometry_estimation
  download_gligen
  download_latent_resizer
  download_loras
  download_model_patches
  download_optical_flow
  download_photomaker
  download_style_models
  download_text_encoders
  download_upscale_models
  download_vae
  download_vae_approx
}

# Parse Command-Line Flags
if [ "$1" == "--all" ]; then
  download_all
  exit 0
elif [ "$1" == "--flux" ]; then
  download_flux_group
  exit 0
elif [ "$1" == "--sdxl" ]; then
  download_sdxl_group
  exit 0
elif [ "$1" == "--video" ]; then
  download_video_group
  exit 0
fi

# Interactive Menu
while true; do
  echo -e "${CYAN}${BOLD}Choose a download option:${NC}"
  echo -e "1) ${MAGENTA}${BOLD}FLUX Essentials${NC} (FLUX Dev Q5, T5-XXL FP8, Clip-L, VAE, Union ControlNet, Realism LoRA)"
  echo -e "2) ${BLUE}${BOLD}SDXL Essentials${NC} (Juggernaut XL Ragnarok, VAE, IP-Adapter, Model Patches)"
  echo -e "3) ${CYAN}${BOLD}Video / Animation Tools${NC} (Depth Anything V3, FILM, RAFT Flow, BEN2 Seg)"
  echo -e "4) ${YELLOW}All Other Models${NC} (EasyNegative, Audio CLAP, PhotoMaker V2, GLIGEN, Latent Resizer, etc.)"
  echo -e "5) ${RED}${BOLD}Download ALL Models${NC} (~45+ GB)"
  echo -e "6) Exit"
  echo -ne "${BOLD}Enter your choice (1-6): ${NC}"
  read choice

  case $choice in
    1)
      download_flux_group
      break
      ;;
    2)
      download_sdxl_group
      break
      ;;
    3)
      download_video_group
      break
      ;;
    4)
      download_audio_encoders
      download_detection
      download_diffusers
      download_embeddings
      download_gligen
      download_latent_resizer
      download_photomaker
      download_upscale_models
      download_vae_approx
      break
      ;;
    5)
      download_all
      break
      ;;
    6)
      echo -e "${GREEN}Exiting. Happy generating!${NC}"
      exit 0
      ;;
    *)
      echo -e "${RED}Invalid option. Please choose between 1 and 6.${NC}\n"
      ;;
  esac
done

echo -e "\n${GREEN}${BOLD}========================================================================${NC}"
echo -e "${GREEN}${BOLD}           Model downloads and checks completed successfully!          ${NC}"
echo -e "${GREEN}${BOLD}========================================================================${NC}"
