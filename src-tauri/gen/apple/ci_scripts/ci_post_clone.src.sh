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

echo "[post-clone] Adding required Rust targets..."
rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios aarch64-apple-darwin x86_64-apple-darwin || true

echo "[post-clone] Installing trunk (ignore if already installed)..."
cargo install trunk --locked || true

echo "[post-clone] Installing tauri-cli (ignore if already installed)..."
cargo install tauri-cli --locked || true

echo "[post-clone] Installing Node dependencies (npm ci)..."
npm ci

echo "[post-clone] Done."
