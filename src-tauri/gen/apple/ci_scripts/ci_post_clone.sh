#!/bin/sh
set -e

echo "Starting Xcode Cloud post-clone setup..."

# Detect CI environment
export CI=1
export XCODE_CLOUD=1

# install rust and cargo
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- --profile minimal --default-toolchain stable -y

. "$HOME/.cargo/env"

# Export PATH for subsequent scripts
echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.bash_profile
echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.zprofile


# install cargo-binstall
curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash

# add necessary targets
rustup target add x86_64-apple-darwin
rustup target add x86_64-apple-ios
rustup target add aarch64-apple-darwin
rustup target add aarch64-apple-ios-sim
rustup target add aarch64-apple-ios


# install trunk
cargo binstall trunk -y

# install tauri-cli with specific version known to work with iOS builds
# Using 2.1.0+ which has better CI support
cargo binstall tauri-cli --version "2.1.0" --locked -y || cargo install tauri-cli --version "2.1.0" --locked


# node and npm (only if not already installed)
if ! command -v node >/dev/null 2>&1; then
  HOMEBREW_NO_AUTO_UPDATE=1
  brew install node
fi

# Navigate to repository root
cd "$CI_PRIMARY_REPOSITORY_PATH"

# Install npm dependencies
echo "Installing npm dependencies..."
npm ci

echo "Post-clone setup complete."
echo "Installed versions:"
rustc --version || true
cargo --version || true
node --version || true
npm --version || true
cargo tauri --version || true
trunk --version || true
