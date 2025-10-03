#!/usr/bin/env bash

set -euo pipefail

echo "[pre-xcodebuild] Starting frontend asset build (trunk release)..."

# Ensure cargo bin (trunk, tauri-cli) is on PATH (idempotent).
export RUSTUP_HOME="${RUSTUP_HOME:-$HOME/.rustup}"
export CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"
export PATH="$HOME/.local/bin:$CARGO_HOME/bin:$PATH"
export VOLTA_HOME="${VOLTA_HOME:-$HOME/.volta}"
export PATH="$VOLTA_HOME/bin:$PATH"




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
