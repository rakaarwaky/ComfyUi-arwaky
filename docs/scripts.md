# Script Utilities Reference

This document provides a comprehensive reference for the shell scripts located in the `scripts/` folder. These scripts automate ROCm driver upgrades, build production assets, extract RPM packages without root access, manage versions, and execute local validation pipelines.

---

## 📖 Table of Contents
1. [Scripts Directory Overview](#scripts-directory-overview)
2. [Detailed Script Reference](#detailed-script-reference)
   - [`upgrade_rocm.sh` (ROCm Installer)](#upgrade_rocmsh-rocm-installer)
   - [`ci-local.sh` (Local CI Runner)](#ci-localsh-local-ci-runner)
   - [`bump-version.sh` (Version Manager)](#bump-versionsh-version-manager)
   - [`install_local.sh` (Rootless RPM Installer)](#install_localsh-rootless-rpm-installer)
   - [`build.sh` (Production Packager)](#buildsh-production-packager)
   - [`run_comfyui.sh` (Standalone Runner)](#run_comfyuish-standalone-runner)
   - [`download_models.sh` (Model Downloader)](#download_modelssh-model-downloader)

---

## Scripts Directory Overview

| Script | Purpose | Needs Root? |
|---|---|---|
| [`scripts/upgrade_rocm.sh`](#upgrade_rocmsh-rocm-installer) | Upgrades host ROCm drivers to 7.2.4 from AMD repositories. | Yes |
| [`scripts/ci-local.sh`](#ci-localsh-local-ci-runner) | Local validation harness (format, checks, clippy, unit tests, shellcheck). | No |
| [`scripts/bump-version.sh`](#bump-versionsh-version-manager) | Bumps the application and backend version strings across the codebase. | No |
| [`scripts/install_local.sh`](#install_localsh-rootless-rpm-installer) | Extracts and installs RPM files locally into the user's `~/.local/` directory. | No |
| [`scripts/build.sh`](#buildsh-production-packager) | Bundles production release binaries (AppImage and RPM packages). | No |
| [`scripts/run_comfyui.sh`](#run_comfyuish-standalone-runner) | Executes the ComfyUI Python backend standalone for isolated debugging. | No |
| [`scripts/download_models.sh`](#download_modelssh-model-downloader) | Automates fetching recommended checkpoints and ControlNet weights. | No |

---

## Detailed Script Reference

### `upgrade_rocm.sh` (ROCm Installer)

An automated setup script tailored for **Fedora 44** to upgrade host drivers to **ROCm 7.2.4** using official AMD RedHat repositories.

* **Execution**:
  ```bash
  sudo bash scripts/upgrade_rocm.sh
  ```
* **Tasks Performed**:
  1. Configures AMD GPU and ROCm yum repositories in `/etc/yum.repos.d/amdgpu.repo`.
  2. Imports the official AMD ROCm GPG signing key.
  3. Removes conflicting standard Fedora package-managed ROCm libraries (`rocm-runtime`, `rocm-smi`, `rocminfo`).
  4. Installs the official AMD library stack (`rocm-runtime`, `rocm-hip-runtime`, `rocm-smi-lib`, `rocminfo`, `hip-runtime-amd`, `rocm-dev`).
  5. Updates environmental variables in the user's `~/.bashrc` file:
     - Sets `ROCM_PATH=/opt/rocm`
     - Appends binaries to `PATH`
     - Configures library lookups in `LD_LIBRARY_PATH`
  6. Automatically checks the host graphics card (via `rocminfo`) and appends the appropriate `HSA_OVERRIDE_GFX_VERSION` to `~/.bashrc` (e.g. `10.3.0` or `11.0.0`) only if the GPU requires it.

---

### `ci-local.sh` (Local CI Runner)

A pre-commit validation script that verifies code quality and formatting before committing changes.

* **Usage**:
  ```bash
  bash scripts/ci-local.sh [options]
  ```
* **Options**:
  - `--fix`: Automatically applies formatting fixes using `cargo fmt` and compiler-suggested adjustments with `cargo clippy --fix`.
  - `--fast`: Skips slower tasks, specifically `shellcheck` analysis and dependency security vulnerability scans (`cargo-audit`).
  - `--skip-audit`: Runs standard checks but skips calling `cargo-audit`.
  - `--skip-shellcheck`: Skips running `shellcheck` on `.sh` scripts.
* **Validation Steps**:
  1. **`cargo fmt`**: Confirms all Rust code meets style requirements.
  2. **`cargo check`**: Compiles the backend shell to verify code integrity.
  3. **`cargo clippy`**: Analyzes the codebase for common lints and warnings.
  4. **`cargo test`**: Runs unit tests.
  5. **`shellcheck`**: Validates all shell scripts in the `scripts/` directory.
  6. **Configuration Check**: Runs a Python parser checks on `src-tauri/tauri.conf.json` to verify valid JSON formatting.
  7. **`cargo-audit`**: Checks the Rust dependency tree against known vulnerability databases (skipped if cargo-audit is not installed).

---

### `bump-version.sh` (Version Manager)

Bumps the application version across the backend manifests, Tauri configurations, and downloader code.

* **Usage**:
  ```bash
  scripts/bump-version.sh <new-version> [options]
  ```
* **Options**:
  - `--backend <version>`: Additionally updates the const `BACKEND_VERSION` inside `src-tauri/src/downloader.rs` to match the target release.
  - `--tag`: Commits all changed manifest files and creates a git tag pointing to `v<new-version>`.
* **Example**:
  ```bash
  # Bump app to 0.2.0, target backend release 1.1.0, and commit + tag the release
  bash scripts/bump-version.sh 0.2.0 --backend 1.1.0 --tag
  ```

---

### `install_local.sh` (Rootless RPM Installer)

Enables users to install the application locally without system-wide administrative privileges (`sudo`).

* **Execution**:
  ```bash
  bash scripts/install_local.sh
  ```
* **How It Works**:
  1. Searches the `dist/` folder or prompt paths for a compiled `.rpm` package matching `comfyui-desktop-*.rpm`.
  2. Creates a temporary extraction folder and extracts the contents using standard RPM commands (`rpm2cpio` and `cpio`).
  3. Moves binary executables into the user's home space path `~/.local/bin/`.
  4. Copies application resource assets (desktop configurations, application icons) into `~/.local/share/`.
  5. Updates local icon paths and updates desktop database registries so ComfyUI Desktop shows up in the user's desktop application menu.

---

### `build.sh` (Production Packager)

Compiles the production-ready distribution packages.

* **Execution**:
  ```bash
  bash scripts/build.sh
  ```
* **Outputs Created**:
  - **AppImage**: A single portable application executable.
  - **RPM**: An installation package for Fedora / RedHat systems.
  All compiled artifacts are stored in the root `dist/` directory.

---

### `run_comfyui.sh` (Standalone Runner)

Executes ComfyUI inside the Python virtual environment directly, bypassing the Tauri wrapper.

* **Execution**:
  ```bash
  bash scripts/run_comfyui.sh
  ```
* **Use Case**:
  Useful for isolating Python runtime problems, resolving package dependencies, or testing custom nodes outside of the Tauri window interface.

---

### `download_models.sh` (Model Downloader)

Automates fetching checkpoints and weights to help users get started.

* **Execution**:
  ```bash
  bash scripts/download_models.sh
  ```
* **Assets Fetched**:
  - Downloads the **Juggernaut XL** SDXL base checkpoint into `ComfyUI/models/checkpoints/`.
  - Downloads the **ControlNet Canny** SDXL model into `ComfyUI/models/controlnet/`.
