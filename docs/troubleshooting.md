# Troubleshooting Handbook

This document provides solutions for common issues encountered when setting up, running, or developing ComfyUI Desktop.

---

## 📖 Table of Contents
1. [Blank or Black Screen on Launch](#blank-or-black-screen-on-launch)
2. [Application Hangs on Shutdown](#application-hangs-on-shutdown)
3. [GPU or ROCm Acceleration Not Detected](#gpu-or-rocm-acceleration-not-detected)
4. [Python Environment & Installation Failures](#python-environment--installation-failures)
5. [Port Conflict / Connection Timeout](#port-conflict--connection-timeout)
6. [Generating Logs & Reporting Bugs](#generating-logs--reporting-bugs)

---

## Blank or Black Screen on Launch

### Symptom
The application window opens, but instead of rendering the loading splash screen or ComfyUI, the interface remains completely blank or black.

### Rationale
ComfyUI Desktop is built on Tauri, which utilizes the system **WebKitGTK** engine on Linux for UI rendering. Under certain AMD graphics architectures (specifically with Mesa drivers), WebKitGTK's default Direct Memory Access Buffer (DMA-BUF) renderer conflicts with the hardware acceleration pathways, resulting in failed page composition.

### Solutions

#### Solution 1: Use Automated Environment Overrides
The application sets these environment overrides automatically. If they are bypassed by your launcher, run the AppImage or executable directly from your terminal with the overrides prepended:
```bash
WEBKIT_DISABLE_DMABUF_RENDERER=1 WEBKIT_FORCE_COMPOSITING_MODE=1 ./ComfyUI-Desktop-*.AppImage
```

#### Solution 2: Disable Hardware Acceleration (Software Rendering Fallback)
If issues persist, disable hardware rendering for WebKitGTK:
```bash
WEBKIT_DISABLE_COMPOSITING_MODE=1 ./ComfyUI-Desktop-*.AppImage
```
*Note: This might result in slightly slower UI transitions and heavier CPU usage.*

---

## Application Hangs on Shutdown

### Symptom
When closing the Tauri app window, the GUI disappears, but the process hangs in the background, keeping port `8188` blocked, or python zombie processes continue to consume CPU cycles.

### Rationale
The Python server spawns child threads or accesses library binaries (such as PyTorch C++ libraries or ROCm workers) that ignore standard termination signals. If the launcher only kills the parent Python process, these children become "zombies" and hold active sockets or file descriptors open.

### Solutions

#### Solution 1: Manual Process Tree Cleanup
Find any residual Python processes associated with ComfyUI and kill them manually:
```bash
# Locate the process group and PIDs
ps aux | grep python3

# Terminate all active python processes associated with the app
killall python3 python
```

#### Solution 2: Force Terminate via PGID (Process Group ID)
If a process group hangs, terminate the entire tree by targeting the negative value of the parent PGID:
```bash
# Replace 12345 with the PID of the parent Python process
kill -9 -12345
```

---

## GPU or ROCm Acceleration Not Detected

### Symptom
ComfyUI launches but runs extremely slowly, or the logs output warning statements like:
`Torch CUDA is not available. Falling back to CPU mode.`

### Rationale
This happens if the host ROCm drivers are missing, PyTorch is misconfigured, or the GPU is a consumer Radeon variant that requires an HSA override.

### Solutions

#### Solution 1: Check HSA Override Output
Verify if the launcher successfully resolved the HSA override for your GPU. Inspect the logs for the following line:
```
[Launcher] HSA_OVERRIDE_GFX_VERSION=10.3.0 (GPU variant requires override)
```
If your card is not officially supported and no override is listed, set it manually in your shell profile:
```bash
# Add to ~/.bashrc or export before launching
export HSA_OVERRIDE_GFX_VERSION=10.3.0   # For RDNA 2 (e.g. RX 6700 XT)
# OR
export HSA_OVERRIDE_GFX_VERSION=11.0.0   # For RDNA 3 (e.g. RX 7600)
```

#### Solution 2: Test PyTorch ROCm Compatibility
Run a smoke test in the project's virtual environment to check PyTorch's hardware access:
```bash
source venv/bin/activate
python -c "import torch; print(torch.cuda.is_available())"
```
If this returns `False`, PyTorch is using a CPU-only package. Re-install the correct ROCm PyTorch wheel from AMD's repository:
```bash
pip uninstall torch torchvision torchaudio -y
pip install -r requirements.txt
```
This installs the correct ROCm 7.2.4 wheels from `repo.radeon.com` (listed in `requirements.txt`). Do not use PyPI torch — it won't have ROCm support.

---

## Python Environment & Installation Failures

### Symptom
Errors occur while running `pip install -r requirements.txt` or the application log reports:
`Failed to start ComfyUI Python process: No such file or directory`

### Solutions

#### Solution 1: Verify Python Version
ComfyUI Desktop requires **Python 3.12**. Using newer (e.g., Python 3.13) or older (e.g., Python 3.10) versions can lead to PyTorch package mismatch or build failures. Check your default version:
```bash
python3 --version
```
If your system's default version differs, create the virtual environment using the explicit version binary:
```bash
python3.12 -m venv venv
```

#### Solution 2: Clear Temp and Reinstall
Corrupted download archives or interrupted pip packages can break environments. Remove the `venv` directory and rebuild it:
```bash
rm -rf venv/
python3.12 -m venv venv
source venv/bin/activate
pip install --upgrade pip
pip install -r requirements.txt
```

---

## Port Conflict / Connection Timeout

### Symptom
The splash loader hangs and eventually triggers a timeout notice:
`Failed to connect to ComfyUI after 60 seconds.`

### Rationale
Another application (such as a standalone ComfyUI runner, or a previously hung background process) is already listening on TCP port `8188`, preventing the new Python server from binding to it.

### Solutions

#### Solution 1: Check for Active Port Listeners
Run a netstat or ss check to see what process is holding port `8188`:
```bash
# Find port owner using ss (requires root/sudo for process names)
sudo ss -tulpn | grep 8188

# Find port owner using lsof
lsof -i :8188
```
If a background process is holding the port, kill it using its PID:
```bash
kill -9 <PID>
```

#### Solution 2: Inspect Initial Startup Logs
Look at the logs printed directly on the splash loader interface. If the Python process crashed immediately due to a syntax error or a missing module, the socket will never open. Refer to the logs to diagnose the specific error.

---

## Generating Logs & Reporting Bugs

If you encounter an unresolved issue, please submit a bug report on GitHub:
1. Open the ComfyUI Desktop application.
2. Click the **"Copy Logs"** button in the startup window. This copies the buffered memory logs (up to 2,000 lines) to your clipboard.
3. Paste the contents into a file or directly into your bug report.
4. When reporting, include:
   - Your Fedora version (e.g. Fedora 44).
   - Your GPU Model (e.g., Radeon RX 6700 XT).
   - Your installed ROCm version (check with `rocm-smi`).
