# AGENTS.md

## What this is

ComfyUI-arwaky is a Tauri v2 desktop wrapper for ComfyUI, targeting **Fedora Linux with AMD ROCm GPUs**. It auto-detects GPU hardware, manages a Python backend process with clean lifecycle shutdown, and provides a splash-screen launcher. The project also includes a standalone model downloader built as an AES (Agentic Engineering System) multi-crate Rust workspace.

## Full project structure

```
ComfyUi-arwaky/
│
├── 📄 AGENTS.md                   # Agent guide (you are here)
├── 📄 README.md                   # User-facing docs
├── 📄 CONTRIBUTING.md             # Contribution guidelines
├── 📄 LICENSE                     # MIT license
├── 📄 comfyui-arwaky              # Interactive script selector menu (bash)
├── 📄 ComfyUI_Downloader.png      # Downloader logo icon
├── 📄 config.yaml                 # User config (HF token, models dir)
│                                    # ⚠ NEVER COMMIT changes to this file
├── 📄 extra_model_paths.yaml      # Custom model paths for ComfyUI
├── 📄 requirements.txt            # Python deps (PyTorch ROCm wheels)
├── 📄 model_requirements.md       # Model requirements guide
│
├── 📁 crates/                     # 🦀 Rust workspace (main source code)
│   ├── 📄 Cargo.toml              # Workspace root (7 members)
│   │
│   ├── 📁 launcher/               # 🖥️ Tauri v2 desktop app
│   │   ├── Cargo.toml
│   │   ├── tauri.conf.json
│   │   ├── build.rs
│   │   ├── 📁 src/                # Rust backend
│   │   │   ├── lib.rs
│   │   │   ├── main.rs
│   │   │   ├── process.rs         # Process group management
│   │   │   ├── gpu.rs             # AMD ROCm auto-detection
│   │   │   ├── logging.rs         # Bounded ring buffer logger
│   │   │   ├── config.rs          # Config loader
│   │   │   └── downloader.rs      # Downloader version const
│   │   ├── 📁 assets/ui/          # Splash screen (HTML/CSS/JS)
│   │   └── 📁 assets/icons/       # App icons (.png)
│   │
│   └── 📁 downloader/             # 📦 AES multi-crate model downloader
│       ├── 📄 Cargo.toml          # Package manifest (lib + 2 bin targets)
│       ├── 📄 lib.rs              # Re-exports sub-crates
│       ├── 📄 root_cli_main_entry.rs    # CLI binary entry point
│       ├── 📄 root_tui_main_entry.rs    # TUI binary entry point
│       ├── 📄 root_downloader_container.rs  # DI container wiring
│       │
│       ├── 📁 shared/             # Layer: taxonomy + contract
│       │   └── src/
│       │       ├── taxonomy_*_vo.rs       # Value objects (Model, Config, Event, Size)
│       │       └── contract_*.rs          # Traits (Port, Protocol, Aggregate)
│       │
│       ├── 📁 config/             # Layer: infrastructure — config
│       │   └── src/
│       │       ├── lib.rs                 # ConfigPort impl
│       │       └── models.json            # 📦 Model registry (all known models)
│       │
│       ├── 📁 downloader-engine/  # Layer: capabilities + infrastructure + agent
│       │   └── src/
│       │       ├── agent_downloader_orchestrator.rs  # Pure orchestrator
│       │       ├── capabilities_download_engine.rs   # Download logic
│       │       └── infrastructure_http_adapter.rs    # HTTP client
│       │
│       ├── 📁 file-utils/         # Layer: capabilities + infrastructure — fileops
│       │   └── src/
│       │       ├── capabilities_file_checker.rs      # File validation logic
│       │       ├── infrastructure_fs_adapter.rs       # Filesystem IO
│       │       └── infrastructure_cache_adapter.rs    # Size cache (url→bytes)
│       │
│       ├── 📁 tui/                # Layer: surface — Ratatui TUI
│       │   └── src/
│       │       ├── surface_tui_state.rs    # App struct + state machine
│       │       ├── surface_tui_actions.rs  # Logging, config, orchestration
│       │       ├── surface_tui_list.rs     # Filter + selection logic
│       │       ├── surface_tui_draw.rs     # Rendering
│       │       ├── surface_tui_event.rs    # Keyboard/mouse input
│       │       └── surface_tui_handler.rs  # TUI init + main loop
│       │
│       └── 📁 cli/                # Layer: surface — CLI
│           └── src/surface_cli_handler.rs  # CLI entry (list/download)
│
├── 📁 scripts/                    # 🐚 Bash automation
│   ├── build.sh                   # Full production build (launcher + downloader)
│   ├── build-launcher.sh          # Build Tauri launcher only
│   ├── build-downloader.sh        # Build downloader CLI + TUI
│   ├── install_local.sh           # Install binaries to ~/.cargo/bin/
│   ├── download_models.sh         # Launch downloader TUI (build + run)
│   ├── ci-local.sh                # Local CI suite
│   ├── bump-version.sh            # Version bump across files
│   ├── run_comfyui.sh             # Run ComfyUI backend standalone
│   ├── install_deps.sh            # Python venv + pip install
│   ├── upgrade_rocm.sh            # ROCm library upgrade
│   └── watch-symlinks.sh          # Symlink health monitor
│
├── 📁 docs/                       # 📚 Documentation
│   ├── architecture.md            # Launcher architecture deep-dive
│   ├── gpu_guide.md               # AMD ROCm setup guide
│   ├── scripts.md                 # Script reference
│   └── troubleshooting.md         # Common issues & fixes
│
├── 📁 dist/                       # 📦 Build output (gitignored)
│   ├── comfyui-desktop            # Launcher binary
│   ├── comfyui-downloader-cli     # Downloader CLI binary
│   ├── comfyui-downloader-tui     # Downloader TUI binary
│   └── SHA256SUMS.txt             # Checksums
│
├── 📁 venv/                       # 🐍 Python virtual env (gitignored)
├── 📁 ComfyUI/                    # 📦 Upstream ComfyUI (git submodule)
├── 📁 input/                      # ComfyUI input dir
├── 📁 output/                     # ComfyUI output dir
├── 📁 user/                       # ComfyUI user data
└── 📁 models/                     # Model download destination (configurable)
```

The downloader follows AES layering:

```
shared (taxonomy + contract)
  ↓
config ─────────────────┐
file-utils ─────────────┤
downloader-engine ──────┤
  ↓                     │
tui / cli (surfaces)    │
  ↓                     │
root_downloader_container (DI wiring)
```

## Development commands

```bash
# Dev mode (hot reload) — launcher only
npx @tauri-apps/cli dev

# Build production bundles → dist/
bash scripts/build.sh

# Build downloader only (CLI + TUI)
bash scripts/build-downloader.sh

# Full CI suite (format, clippy, test, shellcheck, config validation, audit)
bash scripts/ci-local.sh

# Fast CI (skip shellcheck + audit)
bash scripts/ci-local.sh --fast

# Auto-fix format + clippy
bash scripts/ci-local.sh --fix

# Individual checks (workspace-wide)
cargo fmt --manifest-path crates/Cargo.toml --check
cargo clippy --manifest-path crates/Cargo.toml -- -D warnings
cargo test --manifest-path crates/Cargo.toml
shellcheck scripts/*.sh
```

## Python environment

**Python 3.12 required.** Not just any Python — the ROCm wheels are compiled for 3.12 specifically.

```bash
python3.12 -m venv venv
source venv/bin/activate
pip install -r requirements.txt
```

The PyTorch ROCm 7.2.4 wheels come from `repo.radeon.com` (listed in `requirements.txt` via `--extra-index-url`). Do not use PyPI torch — it won't have ROCm support.

## Git submodule

ComfyUI is a submodule. Clone with `--recurse-submodules` or run:

```bash
git submodule update --init --recursive
```

## CI architecture

CI runs on **Fedora 44 containers** (not Ubuntu, despite `ubuntu-24.04` runner). System deps are installed via `dnf`.

Required system packages for Rust/Tauri compilation:
```
webkit2gtk4.1-devel gtk3-devel libappindicator-gtk3-devel
librsvg2-devel patchelf openssl-devel gcc gcc-c++ make pkgconf-pkg-config
```

CI pipeline order: `cargo fmt` → `cargo check` → `cargo clippy` → `cargo test` → `shellcheck` → config validation → `cargo-audit`.

## Version management

Version lives in three files, updated by `scripts/bump-version.sh`:
- `crates/launcher/Cargo.toml` (app version)
- `crates/launcher/tauri.conf.json` (Tauri bundle version)
- `crates/launcher/src/downloader.rs` (`BACKEND_VERSION` constant, optional `--backend` flag)

The downloader workspace has its own `version = "2.0.0"` in `crates/Cargo.toml` (workspace.package).

## Rust conventions

- **Launcher process management**: Child processes use `process_group(0)` for clean group termination via `libc::kill(-pid, ...)`. New sub-processes must follow this pattern.
- **Channel comms**: Use `try_send` (non-blocking) on MPSC channels from reader threads. Never `send` — it can deadlock.
- **Mutex poisoning**: Always handle with `.unwrap_or_else(|p| p.into_inner())` pattern rather than panicking.
- **Logging**: 2000-entry ring buffer, 50-message batches at 100ms flush. Constants in `logging.rs`.
- **Downloader AES architecture**:
  - `contract_*` files = traits only (Send + Sync for thread safety)
  - `capabilities_*` = pure logic implementing protocol traits
  - `infrastructure_*` = IO/network implementing port traits
  - `agent_*_orchestrator` = orchestrator importing only contract traits
  - `surface_*` = frontend (TUI/CLI) calling orchestrator via aggregate only
  - `root_downloader_container.rs` = DI wiring of concrete implementations
- **CLI & TUI binary names**: `comfyui-downloader-cli`, `comfyui-downloader-tui`
- **Install path**: Binaries install to `~/.cargo/bin/` via `install_local.sh`

## Scripts reference

| Script | Purpose |
|--------|---------|
| `scripts/build.sh` | Production build → `dist/` (AppImage + RPM) |
| `scripts/build-downloader.sh` | Build downloader CLI + TUI → `dist/` |
| `scripts/ci-local.sh` | Local CI suite (fmt, clippy, test, shellcheck, audit) |
| `scripts/bump-version.sh` | Bump version across Cargo.toml, tauri.conf.json, downloader.rs |
| `scripts/run_comfyui.sh` | Run ComfyUI backend standalone (no Tauri wrapper) |
| `scripts/download_models.sh` | TUI model downloader (build + launch) |
| `scripts/install_local.sh` | Install binaries to `~/.cargo/bin/` + desktop entries |
| `scripts/upgrade_rocm.sh` | ROCm system library upgrade |
| `scripts/watch-symlinks.sh` | Monitor symlink health |

## Common pitfalls

- `run_comfyui.sh` hardcodes `HSA_OVERRIDE_GFX_VERSION="10.3.0"` — the Rust code in `gpu.rs` auto-detects this correctly. Don't copy the hardcoded value to Rust code.
- `config.yaml` at root contains an HF token (`hf_token`) — never commit changes to this file.
- `dist/` is gitignored — it's build output only.
- The app identifier is `com.arwaky.comfyui` (in `tauri.conf.json`).
- Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/): `feat(scope):`, `fix(scope):`, `chore:`, `ci:`, etc.
- Downloader models live in `crates/downloader/config/models.json`, not in `crates/downloader/`.
- The workspace root is `crates/Cargo.toml`. Build with `--manifest-path crates/Cargo.toml` for workspace-wide operations.
