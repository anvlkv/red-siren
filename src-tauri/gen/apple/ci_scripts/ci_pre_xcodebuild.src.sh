#!/usr/bin/env bash
#
# ci_pre_xcodebuild.src.sh
#
# Source version of the Xcode Cloud pre-xcodebuild script for Red Siren.
# Wrapper at repository root named `ci_pre_xcodebuild.sh` should `source` this file.
#
# Purpose:
#   Prepare frontend (WASM) assets before Xcode builds iOS/mac targets.
#   Purely linear; no conditionals, no environment probing.
#
# Philosophy (MAYA DRY KISS):
#   - Minimal, deterministic.
#   - No branching, no secret / profile checks (Xcode Cloud handles those).
#   - Assume prior script installed rust, trunk, tauri-cli, npm deps.
#
# NOTE:
#   If frontend build customization is needed later (Tailwind pipeline, etc.),
#   extend here but keep it linear (no guards).
#
set -euo pipefail

echo "[pre-xcodebuild] Starting frontend asset build (trunk release)..."

# Ensure cargo bin (trunk, tauri-cli) is on PATH (idempotent).
export CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"
export PATH="$CARGO_HOME/bin:$PATH"

# Build WASM/frontend assets (release mode). Adjust/dist output is handled by trunk defaults
# or any Trunk.toml present in the project.
trunk build --release

echo "[pre-xcodebuild] Frontend assets built."

# (Deliberately nothing else: no warm-up cargo builds per instruction.)
