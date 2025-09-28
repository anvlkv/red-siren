# Red Siren

Red Siren is a noise chime. It pulls the present into focus—a siren's call, loud, brief, true.

## Features

- **Noise-activated**: Responds to your noises in real time
- **Tunable activation**: Adjust the frequencies that trigger responses
- **Siren + chime hybrid**: A cry that blooms like a thousand crystal bowls
- **Open source**: CC-BY-SA licensed

## Requirements

- [Tauri CLI](https://v2.tauri.app/start/prerequisites/): `cargo install tauri-cli`
- [Trunk](https://trunkrs.dev/): `cargo install trunk`
- Node dependencies: `npm install`
- Rust targets: `rustup target add wasm32-unknown-unknown`

### Mobile targets (optional)
```bash
rustup target add aarch64-apple-ios x86_64-apple-ios aarch64-apple-ios-sim
rustup target add aarch64-linux-android armv7-linux-androideabi i686-linux-android x86_64-linux-android
```

## Commands

```bash
# Development
cargo tauri dev

# Build
cargo tauri build

# iOS
cargo tauri ios dev
cargo tauri ios build

# Android
cargo tauri android dev
cargo tauri android build
```

---

Cheers,

a.nvlkv
