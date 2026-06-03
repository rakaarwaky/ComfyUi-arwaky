#!/usr/bin/env bash
# ==============================================================================
# upgrade_rocm.sh — Upgrade ROCm ke versi 7.2.4 dari repo AMD resmi
# Untuk: Fedora 44, GPU AMD RX 6800 XT (gfx1030)
# Jalankan: sudo bash scripts/upgrade_rocm.sh
# ==============================================================================
set -e

ROCM_VERSION="7.2.4"
RHEL_BASE="9.4"

echo "========================================"
echo " ROCm Upgrade ke versi $ROCM_VERSION"
echo "========================================"

# --- 1. Tambah/update repo AMD ---
echo ""
echo "[1/5] Menambahkan repo AMD ROCm $ROCM_VERSION..."
sudo tee /etc/yum.repos.d/amdgpu.repo > /dev/null << EOF
[amdgpu]
name=amdgpu
baseurl=https://repo.radeon.com/amdgpu/latest/rhel/${RHEL_BASE}/main/x86_64/
enabled=1
priority=50
gpgcheck=1
gpgkey=https://repo.radeon.com/rocm/rocm.gpg.key

[rocm]
name=rocm
baseurl=https://repo.radeon.com/rocm/rhel9/${ROCM_VERSION}/main
enabled=1
priority=50
gpgcheck=1
gpgkey=https://repo.radeon.com/rocm/rocm.gpg.key
EOF
echo "✅ Repo ditambahkan."

# --- 2. Import GPG Key ---
echo ""
echo "[2/5] Import GPG key AMD..."
sudo rpm --import https://repo.radeon.com/rocm/rocm.gpg.key
echo "✅ GPG key diimport."

# --- 3. Hapus ROCm lama (Fedora default) ---
echo ""
echo "[3/5] Menghapus ROCm lama dari repo Fedora..."
sudo dnf remove -y rocm-runtime rocm-smi rocminfo rocm-hip-runtime 2>/dev/null || true
echo "✅ ROCm lama dihapus (atau sudah tidak ada)."

# --- 4. Install ROCm 7.2.4 ---
echo ""
echo "[4/5] Menginstall ROCm $ROCM_VERSION..."
sudo dnf install -y \
    rocm-runtime \
    rocm-hip-runtime \
    rocm-smi-lib \
    rocminfo \
    hip-runtime-amd \
    rocm-dev
echo "✅ ROCm $ROCM_VERSION terinstall."

# --- 5. Update LD_LIBRARY_PATH di .bashrc ---
echo ""
echo "[5/5] Mengkonfigurasi environment variables..."

ROCM_PATH="/opt/rocm"
BASHRC="$HOME/.bashrc"

# Tambahkan hanya jika belum ada
if ! grep -q "# ROCm PATH" "$BASHRC"; then
    cat >> "$BASHRC" << 'ENVEOF'

# ROCm PATH
export ROCM_PATH=/opt/rocm
export PATH=$PATH:/opt/rocm/bin:/opt/rocm/rocprofiler/bin
export LD_LIBRARY_PATH=$LD_LIBRARY_PATH:/opt/rocm/lib:/opt/rocm/lib64
ENVEOF
    echo "✅ Environment variables ditambahkan ke ~/.bashrc"

    # Auto-detect GPU and add HSA override only if needed (not for native gfx1030/gfx1100)
    if command -v rocminfo &> /dev/null; then
        GFX_VERSION=$(rocminfo 2>/dev/null | grep -oP 'gfx\K[0-9]+' | head -1)
        case "$GFX_VERSION" in
            1031|1032|1034)
                echo "export HSA_OVERRIDE_GFX_VERSION=10.3.0" >> "$BASHRC"
                echo "ℹ️  HSA override 10.3.0 ditambahkan (GPU: gfx$GFX_VERSION)"
                ;;
            1101|1102|1103)
                echo "export HSA_OVERRIDE_GFX_VERSION=11.0.0" >> "$BASHRC"
                echo "ℹ️  HSA override 11.0.0 ditambahkan (GPU: gfx$GFX_VERSION)"
                ;;
            1030|1100|1200|1201)
                echo "ℹ️  GPU gfx$GFX_VERSION sudah native-supported, tidak perlu HSA override"
                ;;
            *)
                echo "⚠️  GPU gfx$GFX_VERSION tidak dikenali, tidak menambahkan HSA override"
                ;;
        esac
    else
        echo "⚠️  rocminfo belum tersedia, skip deteksi HSA override"
    fi
else
    echo "ℹ️  Environment variables sudah ada di ~/.bashrc"
fi

# --- Verifikasi ---
echo ""
echo "========================================"
echo " Verifikasi Instalasi"
echo "========================================"
echo ""

echo "ROCm version:"
/opt/rocm/bin/rocminfo 2>/dev/null | grep -i "ROCk\|Agent\|gfx" | head -10 || echo "⚠️  rocminfo belum bisa dijalankan, coba logout/login dulu"

echo ""
echo "libroctx64 location:"
find /opt/rocm /usr/lib* -name "libroctx64*" 2>/dev/null || echo "⚠️  Library tidak ditemukan"

echo ""
echo "========================================"
echo "✅ SELESAI! Jalankan: source ~/.bashrc"
echo "   Lalu test: python -c \"import torch; print(torch.cuda.is_available())\""
echo "========================================"
