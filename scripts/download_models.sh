#!/bin/bash
# Script to download and upgrade models for ComfyUI Desktop
# Features: Link validation pre-checking, smart existence checking (skip if present), and size-based download prioritization.

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

# Master list of models: Category | Destination filename | URL | Size in bytes (for sorting) | Group
MODELS=(
  "configs|sd_xl_base.yaml|https://raw.githubusercontent.com/Stability-AI/generative-models/main/configs/inference/sd_xl_base.yaml|3000|sdxl"
  "embeddings|easynegative.safetensors|https://huggingface.co/datasets/gsdf/EasyNegative/resolve/main/EasyNegative.safetensors|24000|other"
  "vae_approx|taesdxl_decoder.pth|https://github.com/madebyollin/taesd/raw/main/taesdxl_decoder.pth|1100000|other"
  "detection|yolov8-dfa-face.pt|https://huggingface.co/Bingsu/adetailer/resolve/main/face_yolov8n.pt|6500000|other"
  "upscale_models|RealESRGAN_x4plus_anime_6B.pth|https://github.com/xinntao/Real-ESRGAN/releases/download/v0.2.2.4/RealESRGAN_x4plus_anime_6B.pth|18000000|other"
  "latent_upscale_models|latent-upscaler-v2.1_SDxl-x1.5.safetensors|https://huggingface.co/city96/SD-Latent-Upscaler/resolve/main/latent-upscaler-v2.1_SDxl-x1.5.safetensors|7400000|other"
  "upscale_models|4x-UltraSharp.pth|https://huggingface.co/lokCX/4x-Ultrasharp/resolve/main/4x-UltraSharp.pth|67000000|other"
  "optical_flow|raft_large_C_T_V2-1bb1363a.pth|https://download.pytorch.org/models/raft_large_C_T_V2-1bb1363a.pth|85000000|video"
  "model_patches|resadapter_v1_sdxl_extralora.safetensors|https://huggingface.co/jiaxiangc/res-adapter/resolve/main/resadapter_v1_sdxl_extrapolation/pytorch_lora_weights.safetensors|1844672|sdxl"
  "frame_interpolation|film_net_fp32.pt|https://huggingface.co/nguu/film-pytorch/resolve/main/film_net_fp32.pt|156000000|video"
  "vae|ae.safetensors|https://huggingface.co/diffusers/FLUX.1-vae/resolve/main/diffusion_pytorch_model.safetensors|167666902|flux"
  "background_removal|ben2.onnx|https://huggingface.co/PramaLLC/BEN2/resolve/main/BEN2_Base.onnx|190000000|video"
  "loras|flux_realism_detailer.safetensors|https://huggingface.co/fal/Realism-Detailer-Kontext-Dev-LoRA/resolve/main/high_detail.safetensors|613109224|flux"
  "clip|clip_l.safetensors|https://huggingface.co/comfyanonymous/flux_text_encoders/resolve/main/clip_l.safetensors|240000000|flux"
  "vae|sdxl_vae.safetensors|https://huggingface.co/madebyollin/sdxl-vae-fp16-fix/resolve/main/sdxl_vae.safetensors|335000000|sdxl"
  "style_models|ip-adapter_sdxl.safetensors|https://huggingface.co/h94/IP-Adapter/resolve/main/sdxl_models/ip-adapter_sdxl.safetensors|1100000000|sdxl"
  "audio_encoders|laion_clap_music.pt|https://huggingface.co/lukewys/laion_clap/resolve/main/music_audioset_epoch_15_esc_90.14.pt|1200000000|other"
  "photomaker|photomaker-v2.bin|https://huggingface.co/TencentARC/PhotoMaker-V2/resolve/main/photomaker-v2.bin|1200000000|other"
  "geometry_estimation|depth_anything_v3_vitl.pth|https://huggingface.co/depth-anything/DA3-LARGE/resolve/main/model.safetensors|1400000000|video"
  "animatediff_models|mm_sdxl_v10_beta.ckpt|https://huggingface.co/guoyww/AnimateDiff/resolve/main/mm_sdxl_v10_beta.ckpt|1600000000|video"
  "controlnet|flux1-dev-controlnet-union.safetensors|https://huggingface.co/Shakker-Labs/FLUX.1-dev-ControlNet-Union-Pro/resolve/main/diffusion_pytorch_model.safetensors|3400000000|flux"
  "gligen|gligen_sd14_textbox_pruned.safetensors|https://huggingface.co/comfyanonymous/GLIGEN_pruned_safetensors/resolve/main/gligen_sd14_textbox_pruned.safetensors|836445074|other"
  "clip_vision|clip_vision_g.safetensors|https://huggingface.co/comfyanonymous/clip_vision_g/resolve/main/clip_vision_g.safetensors|3700000000|sdxl"
  "clip_vision|clip_vision_h.safetensors|https://huggingface.co/Comfy-Org/Wan_2.1_ComfyUI_repackaged/resolve/main/split_files/clip_vision/clip_vision_h.safetensors|3700000000|sdxl"
  "text_encoders|t5xxl_fp8_e4m3fn.safetensors|https://huggingface.co/comfyanonymous/flux_text_encoders/resolve/main/t5xxl_fp8_e4m3fn.safetensors|4900000000|flux"
  "checkpoints|sdxl_lightning_8step.safetensors|https://huggingface.co/ByteDance/SDXL-Lightning/resolve/main/sdxl_lightning_8step.safetensors|6600000000|sdxl"
  "checkpoints|Juggernaut-XL-Ragnarok.safetensors|https://huggingface.co/Drnerdy81/juggernaut-xl-ragnarok/resolve/main/juggernautXL_ragnarokBy.safetensors|6600000000|sdxl"
  "diffusion_models|flux1-dev-Q5_K_S.gguf|https://huggingface.co/city96/FLUX.1-dev-gguf/resolve/main/flux1-dev-Q5_K_S.gguf|8285267232|flux"
  "diffusion_models|flux1-schnell-Q8_0.gguf|https://huggingface.co/city96/FLUX.1-schnell-gguf/resolve/main/flux1-schnell-Q8_0.gguf|10000000000|flux"
  "diffusers|stable-diffusion-xl-base-1.0|https://huggingface.co/stabilityai/stable-diffusion-xl-base-1.0|12000000000|other"
)

# Helper function to check URL validity before downloading
check_links() {
  echo -e "${CYAN}${BOLD}>>> Step 1: Checking URL validity for ALL master list models... <<<${NC}"
  local has_error=false

  # Sort by size (smallest first)
  local sorted_models
  sorted_models=$(printf "%s\n" "${MODELS[@]}" | sort -t'|' -k4 -n)

  while IFS='|' read -r category dest_file url size group; do
    echo -ne "Checking: ${BLUE}$dest_file${NC}... "
    local http_code
    http_code=$(curl -s -o /dev/null -w "%{http_code}" -I -L "$url")

    if [ "$http_code" -eq 200 ] || [ "$http_code" -eq 302 ] || [ "$http_code" -eq 301 ]; then
      echo -e "${GREEN}✓ Valid [HTTP $http_code]${NC}"
    else
      echo -e "${RED}✗ Error [HTTP $http_code]${NC} ($url)"
      has_error=true
    fi
  done <<< "$sorted_models"

  if $has_error; then
    echo -e "\n${RED}${BOLD}ERROR: One or more download URLs are invalid. Aborting script.${NC}"
    exit 1
  else
    echo -e "${GREEN}${BOLD}✓ All links are valid! Proceeding to downloading...${NC}\n"
  fi
}

# Helper function to download folder structure for diffusers
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
      if [ -f "$diffusers_dir/$file" ] && [ -s "$diffusers_dir/$file" ]; then
        echo -e "${GREEN}✓ File already exists: stable-diffusion-xl-base-1.0/$file. Skipping.${NC}"
        continue
      fi
      wget -c --show-progress "https://huggingface.co/stabilityai/stable-diffusion-xl-base-1.0/resolve/main/$file" -O "$diffusers_dir/$file"
    done
  fi
}

# Master download execution group
download_group() {
  local filter_group="$1"
  
  # Step 1: Pre-check link validity
  check_links "$filter_group"

  # Step 2: Download files (sorted by size - smallest first)
  echo -e "${CYAN}${BOLD}>>> Step 2: Downloading files prioritized by size (smallest first) <<<${NC}"
  
  local sorted_models
  sorted_models=$(printf "%s\n" "${MODELS[@]}" | sort -t'|' -k4 -n)

  while IFS='|' read -r category dest_file url size group; do
    if [ -n "$filter_group" ] && [ "$group" != "$filter_group" ]; then
      continue
    fi

    local full_dest_dir="$BASE_DIR/$category"
    local full_dest_path="$full_dest_dir/$dest_file"

    # Specific logic for folder-based downloads like diffusers
    if [ "$category" == "diffusers" ]; then
      if [ -d "$full_dest_path" ] && [ "$(ls -A "$full_dest_path" 2>/dev/null)" ]; then
        echo -e "${GREEN}✓ Folder already exists: diffusers/$dest_file. Skipping.${NC}"
        continue
      fi
      download_diffusers
      continue
    fi

    # Check if file exists and is not empty (Skip if present)
    if [ -f "$full_dest_path" ] && [ -s "$full_dest_path" ]; then
      echo -e "${GREEN}✓ File already exists: $category/$dest_file. Skipping download.${NC}"
      continue
    fi

    # Download file using wget
    mkdir -p "$full_dest_dir"
    echo -e "\n${YELLOW}${BOLD}=== Downloading [$category] $dest_file (~$(numfmt --to=iec-binary --suffix=B "$size")) ===${NC}"
    wget -c --show-progress "$url" -O "$full_dest_path"
    
    if [ $? -eq 0 ]; then
      echo -e "${GREEN}✓ Successfully downloaded/verified $dest_file${NC}"
    else
      echo -e "${RED}✗ Failed to download $dest_file${NC}"
    fi
  done <<< "$sorted_models"
}

# Parse Command-Line Flags
if [ "$1" == "--all" ]; then
  download_group ""
  exit 0
elif [ "$1" == "--flux" ]; then
  download_group "flux"
  exit 0
elif [ "$1" == "--sdxl" ]; then
  download_group "sdxl"
  exit 0
elif [ "$1" == "--video" ]; then
  download_group "video"
  exit 0
elif [ "$1" == "--check" ]; then
  check_links
  exit 0
fi

# Interactive Menu
while true; do
  echo -e "${CYAN}${BOLD}Choose a download option:${NC}"
  echo -e "1) ${MAGENTA}${BOLD}FLUX Essentials${NC} (~28.5 GB) (FLUX Dev Q5, Schnell Q8, T5-XXL FP8, Clip-L, VAE, Union ControlNet, Realism LoRA)"
  echo -e "2) ${BLUE}${BOLD}SDXL Essentials${NC} (~22 GB) (Juggernaut XL Ragnarok, SDXL Lightning, VAE, IP-Adapter + Clip Vision G/H, Model Patches)"
  echo -e "3) ${CYAN}${BOLD}Video / Animation Tools${NC} (~3.4 GB) (Depth Anything V3, FILM, AnimateDiff SDXL, RAFT Flow, BEN2 Seg)"
  echo -e "4) ${YELLOW}All Other Models${NC} (~18 GB) (EasyNegative, Audio CLAP, PhotoMaker V2, GLIGEN, RealESRGAN Anime, Latent Resizer, etc.)"
  echo -e "5) ${RED}${BOLD}Download ALL Models${NC} (~72 GB)"
  echo -e "6) Exit"
  echo -ne "${BOLD}Enter your choice (1-6): ${NC}"
  read choice

  case $choice in
    1)
      download_group "flux"
      break
      ;;
    2)
      download_group "sdxl"
      break
      ;;
    3)
      download_group "video"
      break
      ;;
    4)
      download_group "other"
      break
      ;;
    5)
      download_group ""
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
