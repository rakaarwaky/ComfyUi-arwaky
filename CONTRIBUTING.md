# Contributing to ComfyUI Desktop

Thank you for your interest in contributing! This document provides guidelines and instructions for contributing.

## Development Setup

### Prerequisites

- **Rust** 1.77+ (via [rustup](https://rustup.rs/))
- **Node.js** 18+ (for Tauri CLI)
- **Python 3.12** with ROCm 7.2.4
- **System dependencies** (Ubuntu/Debian):
  ```bash
  sudo apt-get install -y \
    libwebkit2gtk-4.1-dev \
    libgtk-3-dev \
    libappindicator3-dev \
    librsvg2-dev \
    patchelf
  ```

### Getting Started

1. Clone the repository with submodules:
   ```bash
   git clone --recurse-submodules https://github.com/arwaky/comfyui-desktop.git
   cd comfyui-desktop
   ```

2. Set up the Python environment:
   ```bash
   python3.12 -m venv venv
   source venv/bin/activate
   pip install -r requirements.txt
   ```

3. Run in development mode:
   ```bash
   npx @tauri-apps/cli dev
   ```

## Code Standards

### Rust

- Run `cargo fmt --check` before committing
- Run `cargo clippy -- -D warnings` and fix all warnings
- Follow existing code patterns (see `lib.rs` for reference)
- Use `try_send` instead of `send` for MPSC channels
- Handle mutex poisoning gracefully (`Err(poisoned) => poisoned.into_inner()`)

### Commits

- Use clear, descriptive commit messages
- Format: `<type>: <description>` (e.g., `fix: resolve shutdown hang on exit`)
- Types: `feat`, `fix`, `docs`, `style`, `refactor`, `ci`, `chore`

### Pull Requests

1. Create a feature branch from `main`
2. Make your changes following the code standards above
3. Ensure all CI checks pass
4. Update documentation if needed
5. Submit a PR using the provided template

## Architecture Overview

```
src-tauri/src/lib.rs    — All Rust backend logic (process management, log routing, IPC)
src-tauri/assets/ui/    — Splash screen frontend (HTML/CSS/JS)
scripts/                — Helper scripts (build, install, run)
```

### Key Design Decisions

- **MPSC Bounded Channel** — Log routing from reader threads to single writer thread
- **Process Group Kill** — `libc::kill(-pid)` for clean shutdown of entire process tree
- **Monotonic IDs** — Log pagination survives buffer rotation (`pop_front`)
- **Batched IPC emit** — Reduces Tauri event overhead by 10-50x

## Testing

Currently, the project relies on manual testing:

1. `cargo check` — Compilation verification
2. `cargo clippy` — Lint checks
3. `cargo fmt --check` — Format verification
4. Manual launch test — App starts, ComfyUI backend launches, redirect works
5. Shutdown test — Clean exit without hanging

## Reporting Issues

- Use the [Bug Report](https://github.com/arwaky/comfyui-desktop/issues/new?template=bug_report.md) template
- Include OS, GPU, ROCm version, and app version
- Paste relevant log output (use "Copy Logs" button in the app)

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
