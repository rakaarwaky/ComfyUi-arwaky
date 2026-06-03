#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

usage() {
  cat <<EOF
Usage: $(basename "$0") <new-version> [--backend <ver>]

Bump app version in Cargo.toml, tauri.conf.json, and optionally BACKEND_VERSION.

Examples:
  scripts/bump-version.sh 0.2.0
  scripts/bump-version.sh 0.2.0 --backend 1.1.0
  scripts/bump-version.sh 0.2.0 --tag
EOF
  exit 0
}

TAG=false
BACKEND=""
NEW_VER=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    -h|--help) usage ;;
    --backend) BACKEND="$2"; shift 2 ;;
    --tag) TAG=true; shift ;;
    [0-9]*.[0-9]*.[0-9]*)
      if [[ -z "$NEW_VER" ]]; then
        NEW_VER="$1"
        shift
      else
        echo "❌ Multiple versions specified: $NEW_VER and $1"
        exit 1
      fi
      ;;
    -*)
      echo "❌ Unknown option: $1"
      usage
      ;;
    *)
      echo "❌ Unknown argument: $1"
      usage
      ;;
  esac
done

if [[ -z "${NEW_VER:-}" ]]; then
  echo "Error: <new-version> is required"
  echo ""
  usage
fi

if [[ ! "$NEW_VER" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?$ ]]; then
  echo "❌ Invalid version format: $NEW_VER (expected: X.Y.Z or X.Y.Z-pre)"
  exit 1
fi

if [[ ! -f src-tauri/Cargo.toml ]]; then
  echo "❌ src-tauri/Cargo.toml not found. Run from project root."
  exit 1
fi

echo "=== Bumping app version to $NEW_VER ==="

# 1. Cargo.toml (sed-based, no cargo-edit dependency)
echo "[1/3] Updating src-tauri/Cargo.toml..."
if ! sed -i "s/^version = \"[^\"]*\"/version = \"$NEW_VER\"/" src-tauri/Cargo.toml; then
  echo "❌ Failed to update Cargo.toml"
  exit 1
fi
if [ -f src-tauri/Cargo.lock ]; then
  sed -i "0,/^name = \"app\"$/{
    /^name = \"app\"$/,/^version = \"[^\"]*\"$/{
      s/^version = \"[^\"]*\"$/version = \"$NEW_VER\"/
    }
  }" src-tauri/Cargo.lock
fi
ACTUAL_VER=$(grep -m1 '^version' src-tauri/Cargo.toml | sed 's/.*"\(.*\)".*/\1/')
if [ "$ACTUAL_VER" != "$NEW_VER" ]; then
  echo "❌ Version update failed. Expected $NEW_VER, got $ACTUAL_VER"
  exit 1
fi
echo "  ✅ Cargo.toml version: $NEW_VER"

# 2. tauri.conf.json
echo "[2/3] Updating src-tauri/tauri.conf.json..."
if command -v jq &>/dev/null; then
  tmp=$(mktemp)
  jq --arg v "$NEW_VER" '.version = $v' src-tauri/tauri.conf.json > "$tmp" \
    && mv "$tmp" src-tauri/tauri.conf.json \
    && echo "  ✅ tauri.conf.json version: $NEW_VER"
else
  sed -i "s/\"version\": \"[^\"]*\"/\"version\": \"$NEW_VER\"/" src-tauri/tauri.conf.json
  echo "  ✅ tauri.conf.json version: $NEW_VER (sed fallback, install jq for better parsing)"
fi

# 3. BACKEND_VERSION (optional)
if [[ -n "$BACKEND" ]]; then
  echo "[3/3] Updating BACKEND_VERSION to $BACKEND in downloader.rs..."
  sed -i "s/^const BACKEND_VERSION: \&str = \".*\";/const BACKEND_VERSION: \&str = \"$BACKEND\";/" src-tauri/src/downloader.rs
else
  echo "[3/3] Skipped (no --backend)"
fi

# 4. Git tag (optional)
if $TAG; then
  echo "Creating git tag v$NEW_VER..."
  git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/tauri.conf.json
  if [[ -n "$BACKEND" ]]; then
    git add src-tauri/src/downloader.rs
  fi
  git commit -m "chore: bump version to $NEW_VER"
  git tag "v$NEW_VER"
  echo "Tag v$NEW_VER created. Push with: git push origin v$NEW_VER"
fi

echo ""
echo "Done! App version: $NEW_VER${BACKEND:+ (backend: $BACKEND)}"
