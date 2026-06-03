#!/bin/bash
# ComfyUI ROCm Startup Script

# 1. Pastikan driver GPU sudah terikat (bind) jika belum
if [ ! -d "/sys/bus/pci/devices/0000:12:00.0/driver" ]; then
    echo "Peringatan: Driver GPU Radeon RX 6800 tidak aktif. Mencoba mengikat driver..."
    sudo /usr/local/bin/fix-amdgpu.sh
fi

# 2. Arahkan ke direktori kerja parent
cd "$(dirname "$0")"

# 3. Aktifkan Virtual Environment
source venv/bin/activate

# 4. Batasi akses GPU hanya untuk Radeon RX 6800 XT (Device 1)
# Ini untuk mencegah PyTorch mendeteksi integrated graphics (Cezanne APU)
export HIP_VISIBLE_DEVICES="1"

# 5. Masuk ke direktori repositori resmi ComfyUI
cd ComfyUI

# 6. Jalankan ComfyUI
echo "Menjalankan ComfyUI pada Radeon RX 6800 XT..."
python main.py "$@"
