#!/usr/bin/env bash

set -euo pipefail

echo "[pre-xcodebuild] Starting frontend asset build (trunk release)..."

# Ensure cargo bin (trunk, tauri-cli) is on PATH (idempotent).
export RUSTUP_HOME="${RUSTUP_HOME:-$HOME/.rustup}"
export CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"
export PATH="$HOME/.local/bin:$CARGO_HOME/bin:$PATH"
export VOLTA_HOME="${VOLTA_HOME:-$HOME/.volta}"
export PATH="$VOLTA_HOME/bin:$PATH"

# Ensure cargo and Rust toolchain are available (idempotent)
if [ ! -x "$CARGO_HOME/bin/cargo" ]; then
  echo "[pre-xcodebuild] Cargo not found, installing Rust toolchain..."
  curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain stable
fi

# Load cargo environment if present
if [ -f "$CARGO_HOME/env" ]; then
  . "$CARGO_HOME/env"
fi

# Reassert PATH precedence
export PATH="$HOME/.local/bin:$CARGO_HOME/bin:$VOLTA_HOME/bin:$PATH"
# Defensive: ensure Xcode run script expected cargo path exists
EXPECTED_CARGO_BIN="$HOME/.cargo/bin/cargo"
ACTUAL_CARGO_BIN="${CARGO_HOME:-$HOME/.cargo}/bin/cargo"
if [ -x "$ACTUAL_CARGO_BIN" ] && [ "$EXPECTED_CARGO_BIN" != "$ACTUAL_CARGO_BIN" ]; then
  mkdir -p "$(dirname "$EXPECTED_CARGO_BIN")"
  ln -sf "$ACTUAL_CARGO_BIN" "$EXPECTED_CARGO_BIN"
fi

echo "[pre-xcodebuild] Adding required Rust targets..."
rustup target add aarch64-apple-ios aarch64-apple-ios-sim aarch64-apple-darwin x86_64-apple-darwin wasm32-unknown-unknown || true

echo "[pre-xcodebuild] Ensuring CLI tools (tauri-cli, trunk)..."
cargo install tauri-cli --locked || true
cargo install trunk --locked || true




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
