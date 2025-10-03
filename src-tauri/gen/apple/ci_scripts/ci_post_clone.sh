#!/usr/bin/env bash

# ci_post_clone.src.sh
#
# Source version of Xcode Cloud post-clone script for Red Siren.
# Root wrapper 'ci_post_clone.sh' should `source` this file.
#
# Intent: prepare toolchains (Rust, Node deps) with zero branching / checks.
# Philosophy: MAYA DRY KISS — simplest steps, no conditional logic.

set -euo pipefail

echo "[post-clone] Starting toolchain bootstrap (no checks)..."

# Rust toolchain (always invoke installer; it is idempotent if already installed)
export RUSTUP_HOME="${RUSTUP_HOME:-$HOME/.rustup}"
export CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"
curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain stable
. "$CARGO_HOME/env"

# Ensure cargo is available on PATH for Xcode build phase
export PATH="$CARGO_HOME/bin:$PATH"

# Verify cargo installation and set up environment
echo "[post-clone] Verifying cargo installation..."
if [ -f "$CARGO_HOME/bin/cargo" ]; then
    echo "[post-clone] Cargo found at $CARGO_HOME/bin/cargo"
    "$CARGO_HOME/bin/cargo" --version
else
    echo "[post-clone] ERROR: Cargo not found at expected location"
    exit 1
fi



echo "[post-clone] Adding required Rust targets..."
rustup target add aarch64-apple-ios aarch64-apple-ios-sim aarch64-apple-darwin wasm32-unknown-unknown || true

echo "[post-clone] Installing trunk (ignore if already installed)..."
cargo install trunk --locked || true



echo "[post-clone] Installing dependencies (npm ci)..."

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")"/../../../.. && pwd)"
cd "$REPO_ROOT"
npm ci

echo "[post-clone] Setting up tailwindcss wrapper to use npm version..."
# Create a wrapper script that trunk will find before it tries to download its own version
mkdir -p "$HOME/.local/bin"

# Store the current repo root for the wrapper script
WRAPPER_REPO_ROOT="$REPO_ROOT"

# Create the wrapper script
cat > "$HOME/.local/bin/tailwindcss" << EOF
#!/bin/bash
cd "$WRAPPER_REPO_ROOT"
exec npx --yes tailwindcss "\$@"
EOF
chmod +x "$HOME/.local/bin/tailwindcss"

# Add to PATH so trunk finds our wrapper
export PATH="$HOME/.local/bin:$PATH"


echo "[post-clone] Done."
