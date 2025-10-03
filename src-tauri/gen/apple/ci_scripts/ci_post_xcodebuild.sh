#!/usr/bin/env bash
#
# ci_post_xcodebuild.src.sh
#
# Source version of Xcode Cloud post-build script for Red Siren (Tauri).
# The root-level wrapper `ci_post_xcodebuild.sh` should `source` this file.
#
# Purpose (MAYA DRY KISS):
#   - Package produced Apple build artifacts (macOS .app -> zip, surface any .ipa)
#   - Emit simple, deterministic paths for downstream collection.
#   - Absolutely no environment / signing / secret checks (Xcode Cloud handles that).
#
# Notes:
#   - We do NOT notarize, staple, or sign here (Xcode handles signing).
#   - We do NOT fail if artifacts are missing; this script stays lenient.
#   - Adjust patterns later if project structure changes.
#
set -euo pipefail

echo "[post-xcodebuild] Collecting build artifacts (no checks)..."

# Ensure cargo bin path available (harmless if already set).
export CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"
export PATH="$CARGO_HOME/bin:$PATH"

# Working directory anchor (assumes script sourced from repo root wrapper).
WORKROOT="$(pwd)"

# 1. Attempt to locate a macOS .app bundle.
#    We keep it shallow-ish to avoid scanning large toolchains.
MAC_APP_PATH="$(find "${WORKROOT}" -maxdepth 8 -type d -name "*.app" | grep -E "/RedSiren|/tauri-app" | head -n 1 || true)"
if [[ -z "${MAC_APP_PATH}" ]]; then
  # Fallback: first .app of any name (still lenient).
  MAC_APP_PATH="$(find "${WORKROOT}" -maxdepth 8 -type d -name "*.app" | head -n 1 || true)"
fi

# 2. Zip macOS app if found.
MAC_ZIP_NAME="RedSiren-mac.zip"
if [[ -n "${MAC_APP_PATH}" ]]; then
  echo "[post-xcodebuild] Zipping macOS app: ${MAC_APP_PATH} -> ${MAC_ZIP_NAME}"
  # -y (store symlinks as the link), -r recursive
  zip -yr "${MAC_ZIP_NAME}" "${MAC_APP_PATH}" >/dev/null 2>&1 || echo "[post-xcodebuild] Zip attempt finished (non-fatal)."
else
  echo "[post-xcodebuild] No macOS .app bundle found (skipping zip)."
fi

# 3. Locate an iOS .ipa (Xcode Cloud export phase generally produces it when archive/export enabled).
IPA_PATH="$(find "${WORKROOT}" -maxdepth 8 -type f -name "*.ipa" | head -n 1 || true)"

# 4. Print summary (plain lines for easy parsing).
echo
echo "=== Artifact Summary (Post Xcode Build) ==="
[[ -n "${MAC_APP_PATH}" ]] && echo "macOS app bundle: ${MAC_APP_PATH}"
[[ -n "${MAC_APP_PATH}" && -f "${MAC_ZIP_NAME}" ]] && echo "macOS zip: ${WORKROOT}/${MAC_ZIP_NAME}"
[[ -n "${IPA_PATH}" ]] && echo "iOS IPA: ${IPA_PATH}"
echo "==========================================="
echo
echo "[post-xcodebuild] Done."
