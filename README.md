# ComfyUI Desktop

A desktop application shell for [ComfyUI](https://github.com/Comfy-Org/ComfyUI) built with **Tauri v2** and **Rust**. Automatically spawns the ComfyUI Python backend, detects AMD ROCm GPUs, and opens the native ComfyUI web UI inside a desktop webview.

[![CI](https://github.com/arwaky/comfyui-desktop/actions/workflows/ci.yml/badge.svg)](https://github.com/arwaky/comfyui-desktop/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Tauri](https://img.shields.io/badge/Tauri-v2-blue)](https://v2.tauri.app/)
[![ROCm](https://img.shields.io/badge/ROCm-7.2.4-orange)](https://rocm.docs.amd.com/)

---

## Features

- **Automatic GPU Detection** — Selects the discrete GPU with the largest VRAM via `rocm-smi`
- **Graceful Shutdown** — Process group kill (`SIGTERM` → `SIGKILL`) ensures no zombie processes
- **Streaming Logs** — Real-time stdout/stderr piping with batched IPC to the frontend
- **Crash Recovery** — Monitors child process health and emits error events on unexpected exit
- **Port Detection** — Polls `127.0.0.1:8188` and auto-redirects the webview when ComfyUI is ready
- **Bounded Memory** — Log buffer capped at 2,000 entries with monotonic ID pagination
- **Cross-Distro** — AppImage (portable) and RPM (Fedora) packages

---

## Requirements

| Component | Version |
|-----------|---------|
| AMD GPU | RX 6000/7000 series (RDNA 2/3) |
| ROCm | 7.2.4+ |
| Python | 3.12 |
| OS | Linux (Fedora 40+, Ubuntu 24.04+) |

---

## Installation

### AppImage (Portable)

```bash
# Download the latest AppImage
chmod +x ComfyUI-Desktop-*.AppImage
./ComfyUI-Desktop-*.AppImage
```

### RPM (Fedora)

```bash
sudo rpm -i comfyui-desktop-*.rpm
comfyui-desktop
```

### From Source

```bash
# Clone with submodules
git clone --recurse-submodules https://github.com/arwaky/comfyui-desktop.git
cd comfyui-desktop

# Set up Python environment
python3.12 -m venv venv
source venv/bin/activate
pip install -r requirements.txt

# Run in development mode
npx @tauri-apps/cli dev

# Or build release
bash scripts/build.sh
```

---

## Project Structure

```
comfyui-desktop/
├── ComfyUI/                  # ComfyUI submodule (auto-cloned)
├── src-tauri/
│   ├── src/
│   │   ├── main.rs           # Entry point
│   │   ├── lib.rs            # Backend logic (process, logs, IPC, commands)
│   │   └── downloader.rs     # Backend archive download + install
│   ├── assets/
│   │   ├── ui/               # Splash screen (HTML/CSS/JS)
│   │   └── icons/            # Desktop icons
│   ├── Cargo.toml
│   └── tauri.conf.json
├── scripts/
│   ├── build.sh              # Build AppImage + RPM
│   ├── install_local.sh      # Install RPM locally
│   ├── run_comfyui.sh        # Run ComfyUI standalone
│   └── download_models.sh    # Download checkpoint models
├── venv/                     # Python 3.12 + ROCm (gitignored)
├── requirements.txt          # PyTorch ROCm wheels
└── extra_model_paths.yaml    # ComfyUI external model config
```

---

## Architecture

### Process Lifecycle

```
Tauri App (Rust)
  └── spawn: python main.py --extra-model-paths-config ...
        ├── stdout ──→ BufReader thread ──→ MPSC channel
        ├── stderr ──→ BufReader thread ──→ MPSC channel
        └── stdout/stderr piped (not inherited)

MPSC Channel (bounded, 1000 messages)
  └── Writer Thread
        ├── Format: [stdout] / [stderr] / [Launcher]
        ├── Store in LogBuffer (VecDeque, max 2000)
        ├── Batch IPC emit (50 msgs or 100ms)
        └── Console output (debug builds only)

Port Poller Thread
  ├── Check child.try_wait() every 1s
  ├── TcpStream::connect_timeout(127.0.0.1:8188)
  └── On success: navigate webview + set redirect flag

Shutdown Sequence
  1. Set ShutdownSignal (AtomicBool)
  2. Drop MPSC sender
  3. Kill process group: libc::kill(-pid, SIGTERM)
  4. Wait 500ms, then SIGKILL
  5. Join all threads
```

### Key Design Decisions

| Decision | Rationale |
|----------|-----------|
| MPSC bounded channel | Eliminates mutex contention between reader threads |
| Single writer thread | Only one thread touches LogBuffer and emits events |
| Process group (`process_group(0)`) | Allows killing entire subprocess tree |
| Monotonic IDs for log pagination | Survives `pop_front` buffer rotation |
| `recv_timeout` in writer | Ensures batch flush even during idle periods |
| `try_send` in reader threads | Non-blocking, drops messages if channel full |

---

## Scripts

| Script | Description |
|--------|-------------|
| `scripts/build.sh` | Build release AppImage + RPM to `dist/` |
| `scripts/install_local.sh` | Extract RPM and install to `~/.local/` |
| `scripts/run_comfyui.sh` | Run ComfyUI standalone (no wrapper) |
| `scripts/download_models.sh` | Download Juggernaut XL + ControlNet models |

---

## Configuration

### GPU Selection

The app auto-detects the GPU with the largest VRAM. Override with:

```bash
HIP_VISIBLE_DEVICES=1 ./ComfyUI-Desktop-*.AppImage
```

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `HIP_VISIBLE_DEVICES` | Auto-detected | GPU index to use |
| `HSA_OVERRIDE_GFX_VERSION` | Auto-detected from sysfs | ROCm GFX version override (only set when GPU requires it, e.g., RX 6700→`10.3.0`, RX 7600→`11.0.0`; native GPUs like RX 6800 XT / 7900 XTX are left unset) |
| `WEBKIT_DISABLE_DMABUF_RENDERER` | `1` | Prevents WebKitGTK blank screen on AMD GPUs |
| `WEBKIT_FORCE_COMPOSITING_MODE` | `1` | Forces GPU hardware acceleration |

### External Models

Edit `extra_model_paths.yaml` to add custom model directories:

```yaml
external_models:
  base_path: /path/to/your/models
  checkpoints: checkpoints/
  vae: vae/
  loras: loras/
```

---

## Troubleshooting

### App hangs on shutdown

Ensure no other processes are using the ComfyUI port. The app uses process group kill, but external processes holding pipe file descriptors can delay cleanup.

### GPU not detected

Verify ROCm is installed:
```bash
rocm-smi --showmeminfo vram
```

### Black screen on launch

The app sets `WEBKIT_DISABLE_DMABUF_RENDERER=1` to prevent WebKitGTK rendering issues on AMD GPUs. If you still see issues, try:
```bash
WEBKIT_DISABLE_DMABUF_RENDERER=1 WEBKIT_FORCE_COMPOSITING_MODE=1 ./ComfyUI-Desktop-*.AppImage
```

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

---

## License

[MIT](LICENSE)
