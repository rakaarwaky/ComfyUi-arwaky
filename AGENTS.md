# AGENTS.md

## What this is

ComfyUI-arwaky is a Tauri v2 desktop wrapper for ComfyUI, targeting **Fedora Linux with AMD ROCm GPUs**. It auto-detects GPU hardware, manages a Python backend process with clean lifecycle shutdown, and provides a splash-screen launcher. Written in Rust (Tauri) with a bash-based toolchain.

## Project structure

- `crates/launcher/` — Tauri v2 app (Rust backend + splash HTML/CSS/JS)
- `crates/downloader/` — Standalone TUI model downloader (separate Rust binary)
- `ComfyUI/` — Git submodule of upstream ComfyUI Python backend

All CI workflows, scripts, and docs reference `crates/launcher/` as the Rust crate path.

## Development commands

```bash
# Dev mode (hot reload)
npx @tauri-apps/cli dev

# Build production bundles → dist/
bash scripts/build.sh

# Full CI suite (format, clippy, test, shellcheck, config validation, audit)
bash scripts/ci-local.sh

# Fast CI (skip shellcheck + audit)
bash scripts/ci-local.sh --fast

# Auto-fix format + clippy
bash scripts/ci-local.sh --fix

# Individual checks
cargo fmt --manifest-path crates/launcher/Cargo.toml --check
cargo clippy --manifest-path crates/launcher/Cargo.toml -- -D warnings
cargo test --manifest-path crates/launcher/Cargo.toml
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

## Rust conventions

- **Process management**: Child processes use `process_group(0)` for clean group termination via `libc::kill(-pid, ...)`. New sub-processes must follow this pattern.
- **Channel comms**: Use `try_send` (non-blocking) on MPSC channels from reader threads. Never `send` — it can deadlock.
- **Mutex poisoning**: Always handle with `.unwrap_or_else(|p| p.into_inner())` pattern rather than panicking.
- **Logging**: 2000-entry ring buffer, 50-message batches at 100ms flush. Constants in `logging.rs`.

## File layout

```
crates/launcher/
  src/           # Rust: lib.rs (entry), process.rs, gpu.rs, logging.rs, downloader.rs, config.rs
  assets/ui/     # Splash screen HTML/CSS/JS (served by Tauri webview)
  assets/icons/  # App icons for bundles
  tauri.conf.json
  Cargo.toml
  build.rs

crates/downloader/
  src/           # Standalone TUI downloader (ratatui)
  models.json    # Model registry
```

## Scripts reference

| Script | Purpose |
|--------|---------|
| `scripts/build.sh` | Production build → `dist/` (AppImage + RPM) |
| `scripts/ci-local.sh` | Local CI suite (fmt, clippy, test, shellcheck, audit) |
| `scripts/bump-version.sh` | Bump version across Cargo.toml, tauri.conf.json, downloader.rs |
| `scripts/run_comfyui.sh` | Run ComfyUI backend standalone (no Tauri wrapper) |
| `scripts/download_models.sh` | TUI model downloader |
| `scripts/install_local.sh` | Extract RPM to user space (no root needed) |
| `scripts/upgrade_rocm.sh` | ROCm system library upgrade |
| `scripts/watch-symlinks.sh` | Monitor symlink health |

## Common pitfalls

- `run_comfyui.sh` hardcodes `HSA_OVERRIDE_GFX_VERSION="10.3.0"` — the Rust code in `gpu.rs` auto-detects this correctly. Don't copy the hardcoded value to Rust code.
- `config.yaml` at root contains an HF token (`hf_token`) — never commit changes to this file.
- `dist/` is gitignored — it's build output only.
- The app identifier is `com.arwaky.comfyui` (in `tauri.conf.json`).
- Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/): `feat(scope):`, `fix(scope):`, `chore:`, `ci:`, etc.
