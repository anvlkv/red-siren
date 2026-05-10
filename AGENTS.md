# Red Siren Agent Guide

Red Siren is a cross-platform Tauri + Leptos noise musical instrument (Noise Chime).

## Repository Reference
- [README.md](README.md): project commands and top-level project description.
- [REWORK.md](REWORK.md): record of keep/delete decisions and restore links for previous implementation files.
- [Cargo.toml](Cargo.toml): workspace members and shared dependencies.
- [Trunk.toml](Trunk.toml): frontend build/serve configuration.
- [src-tauri/tauri.conf.json](src-tauri/tauri.conf.json): app-level Tauri configuration.
- [styles.css](styles.css): shared theme tokens and base UI styles.

## Crate Purpose
- `src-tauri`: native backend, app lifecycle, platform setup, and Tauri plugin wiring.
- `crates/frontend`: Leptos frontend app shell, routes, pages, and UI components.
- `crates/common`: shared Rust types and cross-crate contracts.
- `crates/audio-system`: audio runtime and signal-processing crate.

## Key File Purpose
- [src-tauri/src/lib.rs](src-tauri/src/lib.rs): backend startup entry and command/plugin registration surface.
- [src-tauri/src/setup/mod.rs](src-tauri/src/setup/mod.rs): backend setup flow.
- [crates/frontend/src/main.rs](crates/frontend/src/main.rs): frontend entry point.
- [crates/frontend/src/app.rs](crates/frontend/src/app.rs): frontend app shell composition.
- [crates/frontend/src/routes.rs](crates/frontend/src/routes.rs): frontend route tree.
- [crates/common/src/lib.rs](crates/common/src/lib.rs): shared module exports.
- [crates/audio-system/src/lib.rs](crates/audio-system/src/lib.rs): audio-system public crate surface.

## Agents
- [.github/agents/architecture-migrator.agent.md](.github/agents/architecture-migrator.agent.md): migration planning and staged delegated execution workflow.

## Skills
- [tailwind-leptos](.github/skills/tailwind-leptos/SKILL.md): frontend styling, theme tokens, and Leptos class patterns.
- [leptos-router-architecture](.github/skills/leptos-router-architecture/SKILL.md): route tree and shell navigation changes.
- [leptos-use-reactivity](.github/skills/leptos-use-reactivity/SKILL.md): browser/reactive utility wiring in frontend utils.
- [tauri-trunk](.github/skills/tauri-trunk/SKILL.md): Tauri + Trunk dev/build/hosting configuration.
- [cpal-runtime](.github/skills/cpal-runtime/SKILL.md): native audio runtime work when audio is reintroduced.
- [fundsp-dsp-design](.github/skills/fundsp-dsp-design/SKILL.md): DSP graph design when synthesis/effects return.