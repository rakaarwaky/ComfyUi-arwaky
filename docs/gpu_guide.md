# AMD ROCm & GPU Setup Guide

This document describes the AMD ROCm compatibility, the backend's automatic GPU detection, the hardware-specific HSA override mechanisms, and environment configurations designed for ComfyUI Desktop.

---

## 📖 Table of Contents
1. [AMD ROCm Compatibility Overview](#amd-rocm-compatibility-overview)
2. [Smart GPU Detection Algorithm](#smart-gpu-detection-algorithm)
3. [Automatic HSA Version Overrides](#automatic-hsa-version-overrides)
4. [Environment Variables Reference](#environment-variables-reference)
5. [Verification & Testing Commands](#verification--testing-commands)

---

## AMD ROCm Compatibility Overview

AMD ROCm (Radeon Open Compute) is the open-source software stack for GPU computing on AMD hardware. To leverage hardware acceleration inside ComfyUI, PyTorch must compile operations targeting the ROCm drivers instead of NVIDIA CUDA.

### Compatible Hardware
* **Officially Supported GPUs**: Radeon RX 6800, RX 6800 XT, RX 6900 XT (gfx1030), Radeon RX 7900 XT, and RX 7900 XTX (gfx1100).
* **Supported via Overrides**: Consumer GPUs such as Radeon RX 6600, RX 6700 XT, RX 7600, RX 7800 XT, and future-proof RDNA 4 cards (gfx12xx series).
* **Minimum Host Driver Stack**: AMD ROCm **7.2.4** or higher.

---

## Smart GPU Detection Algorithm

On systems with multiple graphics cards (e.g. an integrated AMD APU and a discrete AMD Radeon GPU), PyTorch might default to the lower-power integrated GPU, leading to severe out-of-memory (OOM) issues or failure to run models.

To resolve this, ComfyUI Desktop implements an automatic GPU detection algorithm:

```
                  +---------------------------+
                  |  Spawn: rocm-smi          |
                  |  --showmeminfo vram       |
                  +---------------------------+
                                |
                                v
                  +---------------------------+
                  | Parse stdout lines:       |
                  | VRAM Total Memory [GPU ID]|
                  +---------------------------+
                                |
                                v
                  +---------------------------+
                  | Parse VRAM byte count     |
                  | and find highest value    |
                  +---------------------------+
                                |
                                v
                  +---------------------------+
                  | Apply selected GPU ID as  |
                  | HIP_VISIBLE_DEVICES env   |
                  +---------------------------+
```

### Technical Workflow
1. The backend invokes the system command:
   ```bash
   rocm-smi --showmeminfo vram
   ```
2. It parses the stdout lines looking for the `VRAM Total Memory` identifier. A typical output structure is:
   ```
   GPU[0] : VRAM Total Memory (B): 536870912          # Integrated GPU (512MB)
   GPU[1] : VRAM Total Memory (B): 17163091968        # Discrete GPU (16GB)
   ```
3. The parser extracts the GPU index enclosed in brackets `[...]` and converts the VRAM size (in bytes) to a `u64`.
4. It compares all VRAM values and selects the device index with the largest capacity.
5. On subprocess spawning, the launcher sets the environment variable:
   ```bash
   HIP_VISIBLE_DEVICES=<selected_index>
   ```
   This restricts PyTorch to seeing only the discrete high-capacity GPU.

---

## Automatic HSA Version Overrides

ROCm officially targets enterprise Instinct accelerators and high-end gaming GPUs. If you run PyTorch on intermediate consumer Radeon cards (like the RX 6700 XT or RX 7600), PyTorch will crash immediately on startup with a HIP initialization error because it does not recognize the GPU variant.

To solve this, users traditionally have to manually export the `HSA_OVERRIDE_GFX_VERSION` environment variable. ComfyUI Desktop automates this process.

### Hardware Identification
The backend reads the GPU target version directly from the Linux kernel sysfs interface. It checks the topology nodes:
```
/sys/class/kfd/kfd/topology/nodes/node*/gfx_target_version
```

If the sysfs nodes are unavailable, the backend falls back to executing the system command:
```bash
rocminfo
```
It reads the stdout output and parses the first string match containing `gfx` followed by the version digits.

### Override Mapping Rules

The extracted version string (e.g. `1031`, `1102`) is passed to a parser:
* **Native Targets**: Version numbers like `1030` (RDNA 2 native), `1100` (RDNA 3 native), or `1200` (RDNA 4 native) are skipped. No override is applied, letting PyTorch run natively.
* **Patched RDNA 2 Targets**: Sub-versions like `1031` (RX 6700 XT), `1032`, `1033`, or `10.3.1` are automatically mapped to:
  ```bash
  HSA_OVERRIDE_GFX_VERSION=10.3.0
  ```
* **Patched RDNA 3 Targets**: Sub-versions like `1101`, `1102` (RX 7600), `1103`, or `11.0.1` are automatically mapped to:
  ```bash
  HSA_OVERRIDE_GFX_VERSION=11.0.0
  ```
* **Patched RDNA 4 Targets**: Sub-versions like `1201` or `12.0.1` are automatically mapped to:
  ```bash
  HSA_OVERRIDE_GFX_VERSION=12.0.0
  ```
* **Dotted / Unknown Fallbacks**: If the major architecture is detected but the specific sub-version is unknown, it defaults to mapping RDNA 2 to `10.3.0`, RDNA 3 to `11.0.0`, and RDNA 4 to `12.0.0`.

This automatic override prevents startup crashes and ensures immediate compatibility for a wide range of AMD consumer graphics cards.

---

## Environment Variables Reference

When launching ComfyUI, the backend configures several environment variables to stabilize performance and prevent rendering bugs:

* **`HIP_VISIBLE_DEVICES`**
  - **Purpose**: Controls which GPU index PyTorch utilizes.
  - **Rationale**: Isolates the high-VRAM discrete GPU, ignoring integrated APUs.
* **`HSA_OVERRIDE_GFX_VERSION`**
  - **Purpose**: Instructs the ROCm runtime to treat the active GPU as a compatible major architecture target.
  - **Rationale**: Enables PyTorch execution on consumer Radeon RX cards.
* **`WEBKIT_DISABLE_DMABUF_RENDERER` (Set to `1`)**
  - **Purpose**: Disables DMA Buffer sharing in the WebKitGTK renderer.
  - **Rationale**: Resolves a common issue where AMD GPU drivers cause WebKitGTK to render a completely blank or black screen inside Tauri applications.
* **`WEBKIT_FORCE_COMPOSITING_MODE` (Set to `1`)**
  - **Purpose**: Forces hardware-accelerated compositing.
  - **Rationale**: Enhances rendering speed and UI response within the Tauri window.
* **`GIO_USE_PROXY_RESOLVER` (Set to `dummy`) & `no_proxy` (Set to `*`)**
  - **Purpose**: Forces GLib network services to skip proxy searches.
  - **Rationale**: WebKitGTK occasionally spends up to 10 seconds attempting to resolve local proxies on startup, which slows down the splash screen loader.

---

## Verification & Testing Commands

If you suspect ROCm acceleration is not functioning, run the following verification steps in your terminal:

1. **Verify host ROCm status**:
   Ensure `rocm-smi` is installed and reports VRAM:
   ```bash
   rocm-smi --showmeminfo vram
   ```

2. **Verify GPU variant details**:
   Query `rocminfo` to check your GFX target number:
   ```bash
   rocminfo | grep gfx
   ```

3. **Verify PyTorch GPU access**:
   Activate your virtual environment and run the following Python check:
   ```bash
   source venv/bin/activate
   python -c "import torch; print('CUDA/ROCm Available:', torch.cuda.is_available()); print('Device Name:', torch.cuda.get_device_name(0))"
   ```
   *Expected output on a successful setup:*
   ```
   CUDA/ROCm Available: True
   Device Name: Radeon RX 6800 XT
   ```
