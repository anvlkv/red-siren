#!/usr/bin/env bash

set -euo pipefail

echo "[pre-xcodebuild] Starting frontend asset build (trunk release)..."

# Ensure cargo bin (trunk, tauri-cli) is on PATH (idempotent).
export CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"
export PATH="$CARGO_HOME/bin:$PATH"

# Build WASM/frontend assets (release mode). Adjust/dist output is handled by trunk defaults
# or any Trunk.toml present in the project.
# Move to repository root so Trunk.toml is discovered (script runs from ci_scripts/)
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")"/../../../.. && pwd)"
cd "$REPO_ROOT"
trunk build --release

echo "[pre-xcodebuild] Frontend assets built."
