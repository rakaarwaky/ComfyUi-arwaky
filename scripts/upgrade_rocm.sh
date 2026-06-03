#!/usr/bin/env bash
# ==============================================================================
# upgrade_rocm.sh — Setup ROCm 7.2.4 from official AMD repository
# For: Fedora 44, AMD GPU
# Run: sudo bash scripts/upgrade_rocm.sh
#
# This script is IDEMPOTENT — safe to run multiple times.
# Only installs missing components; does not remove/reinstall correct ones.
# ==============================================================================
set -e

ROCM_VERSION="7.2.4"
RHEL_BASE="9.4"

echo "========================================"
echo " ROCm Setup version $ROCM_VERSION"
echo "========================================"

# --- 1. Add/update AMD repo (skip if already exists & matches) ---
echo ""
echo "[1/5] Checking AMD ROCm $ROCM_VERSION repository..."
REPO_FILE="/etc/yum.repos.d/amdgpu.repo"
EXPECTED_BASEURL="https://repo.radeon.com/rocm/rhel9/${ROCM_VERSION}/main"

if [ -f "$REPO_FILE" ] && grep -q "$EXPECTED_BASEURL" "$REPO_FILE"; then
    echo "ℹ️  AMD ROCm $ROCM_VERSION repository already exists, skipping."
else
    echo "   Adding AMD ROCm $ROCM_VERSION repository..."
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
    echo "✅ Repository added."
    sudo rpm --import https://repo.radeon.com/rocm/rocm.gpg.key
    echo "✅ GPG key imported."
fi

# --- 2. Remove ONLY old rocm-runtime from Fedora repo (not AMD) ---
echo ""
echo "[2/5] Checking for old ROCm from Fedora repository..."
if dnf list installed rocm-runtime 2>/dev/null | grep -q "fedora"; then
    echo "   Found rocm-runtime from Fedora repository (old), removing..."
    sudo dnf remove -y rocm-runtime 2>/dev/null || true
    echo "✅ Old rocm-runtime from Fedora repository removed."
else
    echo "ℹ️  No old ROCm from Fedora repository found, skipping."
fi

# --- 3. Install missing ROCm libraries ---
echo ""
echo "[3/5] Checking & installing ROCm libraries required by PyTorch..."

ROCM_PACKAGES=(
    rocm-runtime
    rocm-hip-runtime
    rocm-smi-lib
    rocminfo
    hip-runtime-amd
    rocm-dev
    rccl
    "rccl${ROCM_VERSION}"
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
    rocprim
    "rocprim${ROCM_VERSION}"
    rocthrust
    "rocthrust${ROCM_VERSION}"
    hipcub
    "hipcub${ROCM_VERSION}"
)

# Check which ones are not installed
MISSING=()
for pkg in "${ROCM_PACKAGES[@]}"; do
    if ! rpm -q "$pkg" &>/dev/null; then
        MISSING+=("$pkg")
    fi
done

if [ ${#MISSING[@]} -eq 0 ]; then
    echo "ℹ️  All ROCm libraries are already installed, skipping."
else
    echo "   Packages to be installed: ${MISSING[*]}"
    sudo dnf install -y "${MISSING[@]}"
    echo "✅ ROCm libraries installed."
fi

# --- 4. Update LD_LIBRARY_PATH in .bashrc (skip if already exists) ---
echo ""
echo "[4/5] Configuring environment variables..."
BASHRC="$HOME/.bashrc"

if ! grep -q "# ROCm PATH" "$BASHRC"; then
    cat >> "$BASHRC" << 'ENVEOF'

# ROCm PATH
export ROCM_PATH=/opt/rocm
export PATH=$PATH:/opt/rocm/bin:/opt/rocm/rocprofiler/bin
export LD_LIBRARY_PATH=$LD_LIBRARY_PATH:/opt/rocm/lib:/opt/rocm/lib64
ENVEOF
    echo "✅ Environment variables added to ~/.bashrc"

    if command -v rocminfo &> /dev/null; then
        GFX_VERSION=$(rocminfo 2>/dev/null | grep -oP 'gfx\K[0-9]+' | head -1)
        case "$GFX_VERSION" in
            1031|1032|1034)
                echo "export HSA_OVERRIDE_GFX_VERSION=10.3.0" >> "$BASHRC"
                echo "ℹ️  HSA override 10.3.0 added (GPU: gfx$GFX_VERSION)"
                ;;
            1101|1102|1103)
                echo "export HSA_OVERRIDE_GFX_VERSION=11.0.0" >> "$BASHRC"
                echo "ℹ️  HSA override 11.0.0 added (GPU: gfx$GFX_VERSION)"
                ;;
            1030|1100|1200|1201)
                echo "ℹ️  GPU gfx$GFX_VERSION is natively supported, no HSA override needed"
                ;;
            *)
                echo "⚠️  GPU gfx$GFX_VERSION not recognized"
                ;;
        esac
    fi
else
    echo "ℹ️  Environment variables already exist in ~/.bashrc, skipping."
fi

# --- 5. Verification ---
echo ""
echo "[5/5] Verifying installation..."
echo ""

LIBS=("librccl" "libhipsparse" "librocsparse" "librocblas" "libhipblas" "librocfft" "libhipsolver" "libMIOpen")
for lib in "${LIBS[@]}"; do
    if ldconfig -p 2>/dev/null | grep -q "$lib"; then
        echo "✅ $lib : OK"
    else
        echo "⚠️  $lib not found"
    fi
done

if command -v rocminfo &>/dev/null; then
    GFX=$(rocminfo 2>/dev/null | grep -oP 'gfx[0-9]+' | head -1)
    echo "✅ GPU detected: ${GFX:-not detected}"
else
    echo "⚠️  rocminfo not available"
fi

echo ""
echo "========================================"
echo "✅ DONE!"
echo "   Run : source ~/.bashrc"
echo "   Test: python -c \"import torch; print(torch.cuda.is_available())\""
echo "========================================"
