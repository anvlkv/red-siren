# install rust and cargo
curl https://sh.rustup.rs -sSf | sh -s -- -y
. "$HOME/.cargo/env"            # For sh/bash/zsh/ash/dash/pdksh

# install cargo-binstall
curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash

# use toolchain nightly
rustup toolchain install nightly
rustup default nightly

# add necessary targets
rustup target add x86_64-apple-darwin
rustup target add x86_64-apple-ios
rustup target add aarch64-apple-darwin
rustup target add aarch64-apple-ios-sim
rustup target add aarch64-apple-ios


# install trunk
cargo binstall trunk

# install tauri-cli
cargo binstall tauri-cli --version "^2.0.0" --locked


# node and npm
HOMEBREW_NO_AUTO_UPDATE=1
brew install node

# dependencies
npm ci
