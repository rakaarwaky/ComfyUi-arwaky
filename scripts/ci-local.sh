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
  if eval "$@"; then
    pass "$name"
  else
    fail "$name"
  fi
}

# === Step 1: cargo fmt ===
if $FIX; then
  run_step "cargo fmt (fix)" "cargo fmt --manifest-path src-tauri/Cargo.toml"
else
  run_step "cargo fmt (check)" "cargo fmt --manifest-path src-tauri/Cargo.toml --check"
fi

# === Step 2: cargo check ===
run_step "cargo check" "cargo check --manifest-path src-tauri/Cargo.toml"

# === Step 3: cargo clippy ===
if $FIX; then
  run_step "cargo clippy (fix)" "cargo clippy --manifest-path src-tauri/Cargo.toml --fix --allow-dirty -- -D warnings"
else
  run_step "cargo clippy" "cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings"
fi

# === Step 4: cargo test ===
run_step "cargo test" "cargo test --manifest-path src-tauri/Cargo.toml"

# === Step 5: shellcheck ===
if $FAST || $SKIP_SHELLCHECK; then
  skip "shellcheck (skipped)"
else
  SCRIPT_FILES=$(find scripts/ -name '*.sh' -type f 2>/dev/null || true)
  if [ -n "$SCRIPT_FILES" ]; then
    run_step "shellcheck" "shellcheck $SCRIPT_FILES"
  else
    skip "shellcheck (no .sh files found)"
  fi
fi

# === Step 6: config validation ===
run_step "tauri config validation" "python3 -c 'import json,sys; json.load(open(\"src-tauri/tauri.conf.json\")); print(\"valid\")'"

# === Step 7: cargo-audit (if available) ===
if $FAST || $SKIP_AUDIT; then
  skip "cargo-audit (skipped)"
else
  if command -v cargo-audit &>/dev/null || cargo audit --version &>/dev/null 2>&1; then
    run_step "cargo-audit" "cargo audit --manifest-path src-tauri/Cargo.toml"
  else
    skip "cargo-audit (not installed)"
  fi
fi

echo ""
if [ $EXIT_CODE -eq 0 ]; then
  echo -e "${GREEN}All checks passed!${NC}"
else
  echo -e "${RED}Some checks failed.${NC}"
fi
exit $EXIT_CODE
