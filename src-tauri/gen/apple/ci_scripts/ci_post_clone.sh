# install rust and cargo
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -y -- --profile minimal --default-toolchain nightly


# install cargo-binstall
curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash

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
