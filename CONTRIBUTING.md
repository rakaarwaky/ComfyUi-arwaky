# Contributing to ComfyUI Desktop

Thank you for your interest in contributing to ComfyUI Desktop! This document provides detailed guidelines and instructions to set up your environment, follow project standards, run checks, and submit code changes.

---

## 📖 Table of Contents
1. [Development Setup](#development-setup)
   - [System Prerequisites](#system-prerequisites)
   - [Getting Started](#getting-started)
   - [Development Commands](#development-commands)
2. [Code Standards](#code-standards)
   - [Rust Backend](#rust-backend)
   - [Splash Screen Frontend](#splash-screen-frontend)
   - [Bash Scripts](#bash-scripts)
3. [Commit Message Conventions](#commit-message-conventions)
4. [Pull Request Guidelines](#pull-request-guidelines)
5. [Local CI Verification](#local-ci-verification)
6. [Release and Version Management](#release-and-version-management)
7. [Reporting Issues](#reporting-issues)

---

## Development Setup

### System Prerequisites

To compile and build ComfyUI Desktop, ensure you have the following installed on your host system:

* **Rust Toolchain**: version `1.77+` (via [rustup](https://rustup.rs/))
* **Node.js Runtime**: version `18+` (used for Tauri CLI commands)
* **Python Runtime**: version `3.12`
* **System Libraries** (for Debian/Ubuntu based systems):
  ```bash
  sudo apt-get install -y \
    libwebkit2gtk-4.1-dev \
    libgtk-3-dev \
    libappindicator3-dev \
    librsvg2-dev \
    patchelf \
    shellcheck
  ```
* **System Libraries** (for Fedora based systems):
  ```bash
  sudo dnf install -y \
    webkit2gtk4.1-devel \
    gtk3-devel \
    libappindicator-gtk3-devel \
    librsvg2-devel \
    patchelf \
    ShellCheck
  ```

### Getting Started

1. **Clone the repository recursively** to automatically download the ComfyUI source code sub-module:
   ```bash
   git clone --recurse-submodules https://github.com/rakaarwaky/ComfyUi-arwaky.git
   cd ComfyUi-arwaky
   ```

2. **Configure the Python Environment**:
   Initialize a localized virtual environment and install required PyTorch ROCm wheels:
   ```bash
   python3.12 -m venv venv
   source venv/bin/activate
   pip install -r requirements.txt
   ```

3. **Install Node Packages**:
   ```bash
   npm install
   ```

### Development Commands

To run the application shell with active hot reloading of the backend and splash frontend assets:
```bash
# Run in development mode
npx @tauri-apps/cli dev
```

To compile production AppImages and RPM files to the `dist/` folder:
```bash
bash scripts/build.sh
```

---

## Code Standards

### Rust Backend

All Rust code resides inside the `src-tauri/` folder. Please adhere to the following formatting and compiler standards:

* **Formatting**: Ensure your code is properly formatted before staging changes:
  ```bash
  cargo fmt --manifest-path src-tauri/Cargo.toml --check
  ```
* **Lints**: All warnings must be resolved. Do not commit code that produces compiler warnings. Run the standard linter:
  ```bash
  cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
  ```
* **Concurrence & Safety**:
  - Prefer using bounded, non-blocking operations for channel communications. Use `try_send` instead of blocking `send` inside background thread readers to prevent thread locks.
  - Handle mutex poisoning gracefully:
    ```rust
    let logs = match log_buffer.logs.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    ```
  - Clean up child process trees on termination. Ensure you register new sub-processes in a process group (`process_group(0)`) so the shutdown sequence terminates all children.

### Splash Screen Frontend

The splash screen frontend resides in `src-tauri/assets/ui/`.
* Write clean, semantic HTML5 structure.
* Keep styling inside `styles.css` utilizing css custom variables.
* Javascript logic should avoid bloated frameworks where possible; keep it lightweight, fast, and structured inside `main.js`.
* Ensure that all Tauri API bindings use modern event emission and invocation models.

### Bash Scripts

All script utilities are housed in the `scripts/` folder.
* Add documentation headers indicating the script's purpose and usage.
* Keep script executions secure by enabling safety options at the top:
  ```bash
  set -euo pipefail
  ```
* Check bash scripts using `shellcheck` to catch potential scope leaks or runtime bugs.

---

## Commit Message Conventions

We enforce a semantic commit structure based on [Conventional Commits](https://www.conventionalcommits.org/).

Format:
```
<type>(<scope>): <description>

[optional body]
```

* **Types**:
  - `feat`: A new feature implementation (e.g. `feat(gpu): add automated HSA override for RDNA3`)
  - `fix`: A bug fix (e.g. `fix(shutdown): resolve zombie python processes on termination`)
  - `docs`: Documentation changes only
  - `style`: Changes that do not affect the meaning of the code (formatting, white-space, etc.)
  - `refactor`: A code change that neither fixes a bug nor adds a feature
  - `ci`: CI configuration updates or script modifications
  - `chore`: Version updates, dependency changes, build steps
* **Scope**: Optional, indicating the area of code changed (e.g., `ipc`, `installer`, `scripts`).

---

## Pull Request Guidelines

1. Create a feature branch from the `main` branch: `git checkout -b feat/my-new-feature`
2. Implement your changes following the code standards described above.
3. Verify your changes using the local CI verification script:
   ```bash
   bash scripts/ci-local.sh
   ```
4. Commit your changes with conventional messages.
5. Push to your branch and open a Pull Request against the upstream repository.
6. Provide a detailed summary in your Pull Request explaining the purpose, scope, and testing steps performed.

---

## Local CI Verification

Before staging or submitting code, you must run the local verification suite. This script runs formatting, compilation checks, lints, tests, config validation, and shellcheck:

```bash
# Run all local CI checks
bash scripts/ci-local.sh

# Run checks and automatically apply cargo format and clippy fixes:
bash scripts/ci-local.sh --fix

# Skip slower validation checks (like cargo audit and shellcheck):
bash scripts/ci-local.sh --fast
```

For more info about script arguments, check the [Script Reference](file:///home/raka/App/ComfyUi-arwaky/docs/scripts.md).

---

## Release and Version Management

We manage project versions systematically using custom scripts. If you are preparing a release:

1. Use `bump-version.sh` to update all configuration and Cargo manifests:
   ```bash
   # Bump the app version to 0.2.0
   bash scripts/bump-version.sh 0.2.0
   
   # Bump the app version and specify a backend target release version
   bash scripts/bump-version.sh 0.2.0 --backend 1.0.0
   ```
2. The script updates:
   - App version in `src-tauri/Cargo.toml`
   - Version mapping in `src-tauri/tauri.conf.json`
   - Downloader version constraints in `src-tauri/src/downloader.rs`
3. Optional: Pass `--tag` to automatically create a git tag and commit the updates.

---

## Reporting Issues

* Use the [GitHub Bug Report Template](https://github.com/rakaarwaky/ComfyUi-arwaky/issues/new?template=bug_report.md) for filing bugs.
* Describe your system setup clearly (OS version, GPU model, ROCm version).
* Copy and paste relevant logs by using the "Copy Logs" utility inside the application UI.

---

## License

By contributing, you agree that your contributions will be licensed under the project's [MIT License](LICENSE).
