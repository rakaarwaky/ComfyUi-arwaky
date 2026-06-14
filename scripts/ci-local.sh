#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

FAST=false
FIX=false
SKIP_AUDIT=false
SKIP_SHELLCHECK=false

usage() {
  cat <<EOF
Usage: $(basename "$0") [options]

Options:
  --fast          Skip slow checks (shellcheck, cargo-audit)
  --fix           Run cargo fmt and cargo clippy --fix
  --skip-audit    Skip cargo-audit
  --skip-shellcheck  Skip shellcheck
  -h, --help      Show this help
EOF
  exit 0
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --fast) FAST=true; shift ;;
    --fix) FIX=true; shift ;;
    --skip-audit) SKIP_AUDIT=true; shift ;;
    --skip-shellcheck) SKIP_SHELLCHECK=true; shift ;;
    -h|--help) usage ;;
    *) echo "Unknown option: $1"; exit 1 ;;
  esac
done

EXIT_CODE=0
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

pass() { echo -e "${GREEN}[PASS]${NC} $1"; }
fail() { echo -e "${RED}[FAIL]${NC} $1"; EXIT_CODE=1; }
skip() { echo -e "${YELLOW}[SKIP]${NC} $1"; }

run_step() {
  local name="$1"
  shift
  echo ""
  echo "=== $name ==="
  # shellcheck disable=SC2294
  if eval "$@"; then
    pass "$name"
  else
    fail "$name"
  fi
}

# === Step 1: cargo fmt ===
if $FIX; then
  run_step "cargo fmt launcher (fix)" "cargo fmt --manifest-path crates/launcher/Cargo.toml"
  run_step "cargo fmt downloader (fix)" "cargo fmt --manifest-path crates/downloader/Cargo.toml"
else
  run_step "cargo fmt launcher (check)" "cargo fmt --manifest-path crates/launcher/Cargo.toml --check"
  run_step "cargo fmt downloader (check)" "cargo fmt --manifest-path crates/downloader/Cargo.toml --check"
fi

# === Step 2: cargo check ===
run_step "cargo check launcher" "cargo check --manifest-path crates/launcher/Cargo.toml"
run_step "cargo check downloader" "cargo check --manifest-path crates/downloader/Cargo.toml"

# === Step 3: cargo clippy ===
if $FIX; then
  run_step "cargo clippy launcher (fix)" "cargo clippy --manifest-path crates/launcher/Cargo.toml --fix --allow-dirty -- -D warnings"
  run_step "cargo clippy downloader (fix)" "cargo clippy --manifest-path crates/downloader/Cargo.toml --fix --allow-dirty -- -D warnings"
else
  run_step "cargo clippy launcher" "cargo clippy --manifest-path crates/launcher/Cargo.toml -- -D warnings"
  run_step "cargo clippy downloader" "cargo clippy --manifest-path crates/downloader/Cargo.toml -- -D warnings"
fi

# === Step 4: cargo test ===
run_step "cargo test launcher" "cargo test --manifest-path crates/launcher/Cargo.toml"
run_step "cargo test downloader" "cargo test --manifest-path crates/downloader/Cargo.toml"

# === Step 5: shellcheck ===
if $FAST || $SKIP_SHELLCHECK; then
  skip "shellcheck (skipped)"
else
  if command -v shellcheck &>/dev/null; then
    SCRIPT_FILES=$(find scripts/ -name '*.sh' -type f 2>/dev/null | tr '\n' ' ' || true)
    if [ -n "$SCRIPT_FILES" ]; then
      run_step "shellcheck" "shellcheck $SCRIPT_FILES"
    else
      skip "shellcheck (no .sh files found)"
    fi
  else
    skip "shellcheck (not installed, run: sudo dnf install -y shellcheck)"
  fi
fi

# === Step 6: config validation ===
if command -v jq &>/dev/null; then
  run_step "tauri config validation" "jq empty crates/launcher/tauri.conf.json"
elif command -v python3 &>/dev/null; then
  run_step "tauri config validation" "python3 -c 'import json; json.load(open(\"crates/launcher/tauri.conf.json\")); print(\"valid\")'"
else
  skip "tauri config validation (install jq or python3)"
fi

# === Step 7: cargo-audit (if available) ===
if $FAST || $SKIP_AUDIT; then
  skip "cargo-audit (skipped)"
else
  if cargo audit --version &>/dev/null 2>&1; then
    run_step "cargo-audit launcher" "(cd crates/launcher && cargo audit)"
    run_step "cargo-audit downloader" "(cd crates/downloader && cargo audit)"
  else
    skip "cargo-audit (not installed, run: cargo install cargo-audit --locked)"
  fi
fi

echo ""
if [ $EXIT_CODE -eq 0 ]; then
  echo -e "${GREEN}All checks passed!${NC}"
else
  echo -e "${RED}Some checks failed.${NC}"
fi
exit $EXIT_CODE
