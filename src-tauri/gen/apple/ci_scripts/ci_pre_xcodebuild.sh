#!/usr/bin/env bash

set -euo pipefail

echo "[pre-xcodebuild] Starting frontend asset build (trunk release)..."

# Ensure cargo bin (trunk, tauri-cli) is on PATH (idempotent).
export RUSTUP_HOME="${RUSTUP_HOME:-$HOME/.rustup}"
export CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"
export PATH="$HOME/.local/bin:$CARGO_HOME/bin:$PATH"

# Ensure cargo environment is available for Xcode build phase
echo "[pre-xcodebuild] Setting up cargo environment..."
echo "CARGO_HOME: $CARGO_HOME"
echo "PATH: $PATH"

# Verify cargo is accessible
if [ -x "$CARGO_HOME/bin/cargo" ]; then
    echo "[pre-xcodebuild] cargo found at $CARGO_HOME/bin/cargo"
    "$CARGO_HOME/bin/cargo" --version
elif [ -x "/Users/local/.cargo/bin/cargo" ]; then
    echo "[pre-xcodebuild] cargo found at /Users/local/.cargo/bin/cargo"
    /Users/local/.cargo/bin/cargo --version
else
    echo "[pre-xcodebuild] WARNING: cargo not found at expected locations"
    which cargo || echo "cargo not in PATH"
fi

# Build WASM/frontend assets (release mode). Adjust/dist output is handled by trunk defaults
# or any Trunk.toml present in the project.
# Move to repository root so Trunk.toml is discovered (script runs from ci_scripts/)
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")"/../../../.. && pwd)"
cd "$REPO_ROOT"

echo "[pre-xcodebuild] Verifying tailwindcss wrapper is available..."
if [ -x "$HOME/.local/bin/tailwindcss" ]; then
    echo "[pre-xcodebuild] tailwindcss wrapper found, using npm version"
    "$HOME/.local/bin/tailwindcss" --version
else
    echo "[pre-xcodebuild] WARNING: tailwindcss wrapper not found, trunk may download default version"
fi

trunk build --release

echo "[pre-xcodebuild] Frontend assets built."
