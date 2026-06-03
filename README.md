# ComfyUI Desktop (Radeon RX 6800 XT ROCm Edition)

A premium desktop application shell wrapping **ComfyUI** using **Tauri v2** and **Rust**. The app automatically spawns the ComfyUI Python backend, targets the Radeon RX 6800 XT GPU via ROCm 7.2.4, monitors startup, and opens the native ComfyUI web UI inside a desktop webview window.

---

## Folder Structure

To keep the repository clean and avoid accidental deletion of ComfyUI models or Python environments, all Tauri source code and configurations are isolated inside the `src-tauri` directory.

```
/home/raka/App/ComfyUi-arwaky/
├── ComfyUI/                 # Official ComfyUI repository submodule
├── src-tauri/               # Isolated Rust/Tauri project files
│   ├── src/                 # Rust backend code (main.rs, lib.rs)
│   ├── assets/              # Frontend UI loading page & desktop icons
│   │   ├── ui/              # HTML/CSS/JS for the futuristic splash screen
│   │   └── icons/           # Desktop application icons
│   ├── Cargo.toml           # Rust package configuration
│   └── tauri.conf.json      # Tauri application configuration (inline capabilities)
├── scripts/                 # Shell helper scripts (download_models.sh, run_comfyui.sh)
├── venv/                    # Local Python virtual environment (Python 3.12)
├── requirements.txt         # Root-level requirements linking ROCm 7.2 wheels
└── README.md                # This documentation file
```

---

## Python Virtual Environment Setup

The Python environment is managed using Python 3.12 and pre-compiled ROCm 7.2.4 wheels from AMD's repository.

### Recreating the venv
If you ever need to recreate the environment, run:

```bash
# 1. Create a Python 3.12 virtual environment
python3.12 -m venv venv

# 2. Activate the environment
source venv/bin/activate

# 3. Upgrade pip
pip install --upgrade pip

# 4. Install PyTorch ROCm 7.2 and ComfyUI dependencies
pip install -r requirements.txt
```

---

## Desktop Integration (Fedora Linux)

To integrate this application directly with your system's application launcher menu, a desktop entry is created at:
`~/.local/share/applications/comfyui-desktop.desktop`

### Launching the App
- **From Application Menu**: Press the `Super` (Windows) key, search for **"ComfyUI Desktop"**, and hit enter.
- **From Terminal**:
  ```bash
  comfyui-desktop
  ```

---

## Development & Build Commands

### Run in Development Mode
Builds and runs the wrapper locally with live terminal logging:
```bash
npx @tauri-apps/cli dev
```

### Build Fedora Installer Package (.rpm)
Compiles the application in release mode and packages it as a Fedora installer (`.rpm`):
```bash
npx @tauri-apps/cli build
```
The output `.rpm` file will be generated in:
`target/release/bundle/rpm/comfyui-desktop-0.1.0-1.x86_64.rpm`
