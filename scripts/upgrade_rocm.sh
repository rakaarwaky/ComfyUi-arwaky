#!/bin/bash
set -euo pipefail

step() {
  echo "==> $*"
}

fail() {
  echo "ERROR: $*" >&2
  exit 1
}

usage() {
  cat <<'EOF'
Usage: sudo bash upgrade_rocm.sh [--version 7.2.4]

This script expects Fedora/RHEL-family dnf behavior.
EOF
}

ROCM_VERSION=""
while [ $# -gt 0 ]; do
  case "$1" in
    --version)
      ROCM_VERSION="${2:-}"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage
      exit 1
      ;;
  esac
done

ROCM_VERSION="${ROCM_VERSION:-7.2.4}"
ROCM_YUM_REPO_URL="https://repo.radeon.com/rocm/rhel9/${ROCM_VERSION}/main"
REPO_FILE="/etc/yum.repos.d/rocm.repo"
REPO_FILE_DIR="/etc/yum.repos.d"
REPO_FILE_BASENAME="$(basename "$REPO_FILE")"

if [ "$(id -u)" != "0" ]; then
  fail "This script must be run as root."
fi

step "Refreshing package metadata"
dnf makecache -y >/dev/null || fail "Unable to refresh dnf metadata"

step "Ensuring ROCm release repo"
mkdir -p "$REPO_FILE_DIR"
if [ ! -f "$REPO_FILE" ]; then
  cat > "$REPO_FILE" <<EOF
[ROCm]
name=ROCm
baseurl=${ROCM_YUM_REPO_URL}
enabled=1
gpgcheck=1
gpgkey=https://repo.radeon.com/rocm/rocm.gpg.key
EOF
fi

step "Removing repo files that do not match the requested ROCm version"
while IFS= read -r repo_file; do
  [ -n "$repo_file" ] || continue
  if [ "$(basename "$repo_file")" = "$REPO_FILE_BASENAME" ]; then
    continue
  fi
  if grep -Eq 'rocm|amdgpu' "$repo_file" 2>/dev/null; then
    rm -f "$repo_file"
  fi
done < <(find "$REPO_FILE_DIR" -maxdepth 1 -type f 2>/dev/null | sort)

step "Removing old ROCm/runtime Fedora packages"
readarray -t conflict_packages < <(rpm -qa 'rocm-runtime*' 'hip-*' 'hip*-runtime*' 2>/dev/null | sort || true)
if [ "${#conflict_packages[@]}" -gt 0 ]; then
  removed=()
  for pkg in "${conflict_packages[@]}"; do
    repo="$(dnf repoquery --installed --queryformat '%{repoid}' "$pkg" 2>/dev/null | grep -Fx "fedora" || true)"
    if [ -n "$repo" ]; then
      removed+=("$pkg")
    fi
  done
  if [ "${#removed[@]}" -gt 0 ]; then
    dnf remove -y "${removed[@]}"
  fi
fi

step "Installing AMD ROCm ${ROCM_VERSION} packages"
if ! dnf install -y rocm-hip-libs rocm-smi rocm-comgr rocm-dev; then
  fail "Required ROCm package install failed"
fi

echo "ROCm verification is not implemented as an auto-check in this script."
