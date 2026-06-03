#!/usr/bin/env bash
# ==============================================================================
# upgrade_rocm.sh — Setup ROCm 7.2.4 dari repo AMD resmi
# Untuk: Fedora 44, GPU AMD
# Jalankan: sudo bash scripts/upgrade_rocm.sh
#
# Script ini IDEMPOTENT — aman dijalankan berkali-kali.
# Hanya install yang belum ada, tidak hapus-install ulang yang sudah benar.
# ==============================================================================
set -e

ROCM_VERSION="7.2.4"
RHEL_BASE="9.4"

echo "========================================"
echo " ROCm Setup versi $ROCM_VERSION"
echo "========================================"

# --- 1. Tambah/update repo AMD (skip jika sudah ada & sama) ---
echo ""
echo "[1/5] Mengecek repo AMD ROCm $ROCM_VERSION..."
REPO_FILE="/etc/yum.repos.d/amdgpu.repo"
EXPECTED_BASEURL="https://repo.radeon.com/rocm/rhel9/${ROCM_VERSION}/main"

if [ -f "$REPO_FILE" ] && grep -q "$EXPECTED_BASEURL" "$REPO_FILE"; then
    echo "ℹ️  Repo AMD ROCm $ROCM_VERSION sudah ada, skip."
else
    echo "   Menambahkan repo AMD ROCm $ROCM_VERSION..."
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
    sudo rpm --import https://repo.radeon.com/rocm/rocm.gpg.key
    echo "✅ GPG key diimport."
fi

# --- 2. Hapus HANYA rocm-runtime lama dari repo Fedora (bukan AMD) ---
echo ""
echo "[2/5] Mengecek ROCm lama dari repo Fedora..."
if dnf list installed rocm-runtime 2>/dev/null | grep -q "fedora"; then
    echo "   Ditemukan rocm-runtime dari repo Fedora (lama), menghapus..."
    sudo dnf remove -y rocm-runtime 2>/dev/null || true
    echo "✅ rocm-runtime lama dari repo Fedora dihapus."
else
    echo "ℹ️  Tidak ada ROCm lama dari repo Fedora, skip."
fi

# --- 3. Install library ROCm yang belum ada ---
echo ""
echo "[3/5] Mengecek & install library ROCm yang dibutuhkan PyTorch..."

ROCM_PACKAGES=(
    rocm-runtime
    rocm-hip-runtime
    rocm-smi-lib
    rocminfo
    hip-runtime-amd
    rocm-dev
    hipsparse
    "hipsparse${ROCM_VERSION}"
    rocsparse
    "rocsparse${ROCM_VERSION}"
    rocblas
    "rocblas${ROCM_VERSION}"
    hipblas
    "hipblas${ROCM_VERSION}"
    hipblaslt
    "hipblaslt${ROCM_VERSION}"
    rocfft
    "rocfft${ROCM_VERSION}"
    hipsolver
    "hipsolver${ROCM_VERSION}"
    miopen-hip
    "miopen-hip${ROCM_VERSION}"
    comgr
    "comgr${ROCM_VERSION}"
)

# Cek mana yang belum terinstall
MISSING=()
for pkg in "${ROCM_PACKAGES[@]}"; do
    if ! dnf list installed "$pkg" &>/dev/null; then
        MISSING+=("$pkg")
    fi
done

if [ ${#MISSING[@]} -eq 0 ]; then
    echo "ℹ️  Semua library ROCm sudah terinstall, skip."
else
    echo "   Package yang perlu diinstall: ${MISSING[*]}"
    sudo dnf install -y "${MISSING[@]}"
    echo "✅ Library ROCm terinstall."
fi

# --- 4. Update LD_LIBRARY_PATH di .bashrc (skip jika sudah ada) ---
echo ""
echo "[4/5] Mengkonfigurasi environment variables..."
BASHRC="$HOME/.bashrc"

if ! grep -q "# ROCm PATH" "$BASHRC"; then
    cat >> "$BASHRC" << 'ENVEOF'

# ROCm PATH
export ROCM_PATH=/opt/rocm
export PATH=$PATH:/opt/rocm/bin:/opt/rocm/rocprofiler/bin
export LD_LIBRARY_PATH=$LD_LIBRARY_PATH:/opt/rocm/lib:/opt/rocm/lib64
ENVEOF
    echo "✅ Environment variables ditambahkan ke ~/.bashrc"

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
                echo "⚠️  GPU gfx$GFX_VERSION tidak dikenali"
                ;;
        esac
    fi
else
    echo "ℹ️  Environment variables sudah ada di ~/.bashrc, skip."
fi

# --- 5. Verifikasi ---
echo ""
echo "[5/5] Verifikasi instalasi..."
echo ""

if ldconfig -p 2>/dev/null | grep -q "libhipsparse"; then
    echo "✅ libhipsparse  : OK"
else
    echo "⚠️  libhipsparse tidak ditemukan"
fi

if command -v rocminfo &>/dev/null; then
    GFX=$(rocminfo 2>/dev/null | grep -oP 'gfx[0-9]+' | head -1)
    echo "✅ GPU terdeteksi : ${GFX:-tidak terdeteksi}"
else
    echo "⚠️  rocminfo belum tersedia"
fi

echo ""
echo "========================================"
echo "✅ SELESAI!"
echo "   Jalankan: source ~/.bashrc"
echo "   Test    : python -c \"import torch; print(torch.cuda.is_available())\""
echo "========================================"
