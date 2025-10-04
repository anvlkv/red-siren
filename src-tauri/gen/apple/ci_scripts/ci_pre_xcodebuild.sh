#!/bin/sh
# Purpose: run trunk build from the repo root (where Trunk.toml lives) and ensure trunk is on PATH.
# Why: Xcode’s pre-build environment may not source Cargo env or start in the correct directory.

set -eu

# 1) Ensure Cargo bin dir is on PATH so `trunk` is resolvable.
if [ -f "$HOME/.cargo/env" ]; then
  # shellcheck disable=SC1090
  . "$HOME/.cargo/env"
else
  export PATH="$HOME/.cargo/bin:$PATH"
fi

# Symlink Cargo-related binaries into /usr/local/bin so Xcode build phases can find them
BIN_DIR="$HOME/.cargo/bin"
DEST_DIR="/usr/local/bin"
mkdir -p "$DEST_DIR"
for bin in cargo rustc rustup tauri cargo-tauri trunk; do
  if [ -x "$BIN_DIR/$bin" ]; then
    ln -sf "$BIN_DIR/$bin" "$DEST_DIR/$bin" || true
  fi
done

# 2) Find the repository root (directory containing Trunk.toml).
SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)

find_trunk_root() {
  start_dir="$1"
  dir="$start_dir"
  while [ "$dir" != "/" ] && [ -n "$dir" ]; do
    if [ -f "$dir/Trunk.toml" ]; then
      printf "%s\n" "$dir"
      return 0
    fi
    dir=$(dirname "$dir")
  done
  return 1
}

REPO_ROOT=""

# a) If in a git repo, prefer git top-level (if it contains Trunk.toml)
if command -v git >/dev/null 2>&1; then
  if GIT_TOP=$(git rev-parse --show-toplevel 2>/dev/null || true); then
    if [ -n "$GIT_TOP" ] && [ -f "$GIT_TOP/Trunk.toml" ]; then
      REPO_ROOT="$GIT_TOP"
    fi
  fi
fi

# b) Fallback: walk up from the script location
if [ -z "${REPO_ROOT:-}" ]; then
  if ROOT_FROM_SCRIPT=$(find_trunk_root "$SCRIPT_DIR"); then
    REPO_ROOT="$ROOT_FROM_SCRIPT"
  fi
fi

# c) Last resort: walk up from current working directory
if [ -z "${REPO_ROOT:-}" ]; then
  if ROOT_FROM_CWD=$(find_trunk_root "$PWD"); then
    REPO_ROOT="$ROOT_FROM_CWD"
  fi
fi

if [ -z "${REPO_ROOT:-}" ]; then
  echo "Error: Could not locate repository root (Trunk.toml not found)."
  echo "PWD: $PWD"
  echo "SCRIPT_DIR: $SCRIPT_DIR"
  exit 1
fi

# Detect and rebuild cargo-tauri if CPU arch mismatches (fixes "Bad CPU type in executable")
CARGO_TAURI_BIN="$HOME/.cargo/bin/cargo-tauri"
if [ -x "$CARGO_TAURI_BIN" ]; then
  HOST_ARCH="$(uname -m)"
  BIN_INFO="$(file -b "$CARGO_TAURI_BIN" 2>/dev/null || true)"
  NEED_REBUILD=0
  case "$HOST_ARCH" in
    arm64)
      case "$BIN_INFO" in
        *arm64*) ;;
        *) NEED_REBUILD=1 ;;
      esac
      ;;
    x86_64)
      case "$BIN_INFO" in
        *x86_64*) ;;
        *) NEED_REBUILD=1 ;;
      esac
      ;;
  esac
  if [ "$NEED_REBUILD" -eq 1 ]; then
    echo "cargo-tauri arch mismatch: host=$HOST_ARCH, bin='$BIN_INFO'. Rebuilding tauri-cli natively..."
    cargo install tauri-cli --version "^2.0.0" --locked -f
    # Refresh symlink for Xcode
    if [ -x "$BIN_DIR/cargo-tauri" ]; then
      ln -sf "$BIN_DIR/cargo-tauri" "$DEST_DIR/cargo-tauri" || true
    fi
  fi
fi

cd "$REPO_ROOT"

# Pin Trunk to the Tailwind version matching package.json to avoid mismatches
export TRUNK_TOOLS_TAILWINDCSS="4.1.13"

# Create a shim so Trunk uses the project's Tailwind CLI instead of its cached binary
# Determine Trunk cache directory (macOS default, XDG if set, Linux fallback)
if [ -n "${XDG_CACHE_HOME:-}" ]; then
  TRUNK_CACHE_DIR="$XDG_CACHE_HOME/dev.trunkrs.trunk"
elif [ "$(uname)" = "Darwin" ]; then
  TRUNK_CACHE_DIR="$HOME/Library/Caches/dev.trunkrs.trunk"
else
  TRUNK_CACHE_DIR="$HOME/.cache/dev.trunkrs.trunk"
fi

TW_VER="${TRUNK_TOOLS_TAILWINDCSS:-4.1.13}"
TW_DIR="$TRUNK_CACHE_DIR/tailwindcss-$TW_VER"
TW_PATH="$TW_DIR/tailwindcss"
mkdir -p "$TW_DIR"

# Write shim that delegates to the project's node_modules Tailwind CLI
cat > "$TW_PATH" <<EOF
#!/bin/sh
exec "$REPO_ROOT/node_modules/.bin/tailwindcss" "\$@"
EOF
chmod +x "$TW_PATH"

# Prepend project-local Node bin to PATH so Trunk uses the local Tailwind CLI
export PATH="$REPO_ROOT/node_modules/.bin:$PATH"

# Verify Tailwind resolves to project-local binary
TAILWIND_BIN="$(command -v tailwindcss || true)"
if [ -z "$TAILWIND_BIN" ]; then
  echo "Warning: tailwindcss not found on PATH after adding node_modules/.bin"
else
  echo "Using tailwindcss at: $TAILWIND_BIN"
  case "$TAILWIND_BIN" in
    "$REPO_ROOT"/node_modules/*) ;;
    *)
      echo "Warning: tailwindcss is not resolving from project node_modules; current: $TAILWIND_BIN"
      ;;
  esac
  tailwindcss --version || true
fi

# Workaround: pre-create Tauri CLI dev server addr file to avoid panic in Xcode Cloud
# See error: failed to read missing addr file /Volumes/workspace/tmp/com.anvlkv.red-siren.app-server-addr
TMP_BASE="${TMPDIR:-/tmp}"
if [ -d "/Volumes/workspace/tmp" ]; then
  TMP_BASE="/Volumes/workspace/tmp"
fi
ADDR_FILE="$TMP_BASE/com.anvlkv.red-siren.app-server-addr"
mkdir -p "$TMP_BASE"
if [ ! -f "$ADDR_FILE" ]; then
  echo "127.0.0.1:65532" > "$ADDR_FILE"
  echo "Created Tauri addr file at: $ADDR_FILE"
else
  echo "Tauri addr file exists: $ADDR_FILE"
fi

# 3) Verify trunk is available.
if ! command -v trunk >/dev/null 2>&1; then
  echo "Error: trunk is not on PATH. Ensure it was installed (post-clone) and Cargo env is sourced."
  echo "PATH=$PATH"
  exit 127
fi

# Optional: show trunk version for logs
trunk --version || true

# 4) Build
trunk build --release
