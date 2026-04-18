# Red Siren Agent Guide

Red Siren is a cross-platform Tauri + Leptos noise-chime instrument. Treat the product model as evolving: preserve existing architecture, but avoid hard-coding assumptions from stale comments or WIP behavior without checking the current code path first.

## Start Here
- Read [README.md](README.md) for the documented dev and build commands.
- Read [src-tauri/src/lib.rs](src-tauri/src/lib.rs) to see the backend startup flow, plugin setup, and the full Tauri command surface.
- Read [crates/frontend/src/app.rs](crates/frontend/src/app.rs) and [crates/frontend/src/routes.rs](crates/frontend/src/routes.rs) for the frontend shell, theme setup, and route structure.
- Read [crates/audio-system/README.md](crates/audio-system/README.md) for the stable audio runtime boundaries.
- Read [crates/audio-system/src/system/README.md](crates/audio-system/src/system/README.md) before changing DSP graph wiring.

## Workspace Shape
- [Cargo.toml](Cargo.toml) defines a Rust workspace with `crates/audio-system`, `crates/common`, `crates/frontend`, and `src-tauri`.
- `src-tauri` is the Tauri backend and app lifecycle layer.
- `crates/frontend` is the Leptos CSR frontend compiled with Trunk.
- `crates/audio-system` owns DSP, runtime backends, telemetry, quality gating, and snoops.
- `crates/common` holds shared types, commands, events, and generated constants shared across frontend and backend.

## Build And Run
- Use the commands documented in [README.md](README.md): `cargo tauri dev`, `cargo tauri build`, and the mobile `cargo tauri ios/android` variants.
- Frontend hosting is Trunk-driven. Keep [Trunk.toml](Trunk.toml) and [src-tauri/tauri.conf.json](src-tauri/tauri.conf.json) aligned when changing ports, build commands, or frontend output paths.
- Do not assume useful npm scripts exist. The repo uses `npm install` for Tailwind tooling, not a full JS app pipeline.

## Architecture Notes
- The backend command registry in [src-tauri/src/lib.rs](src-tauri/src/lib.rs) is the fastest map of app capabilities. New user-facing features usually need coordinated changes in `common`, `src-tauri`, and `crates/frontend`.
- The audio engine is intentionally separated from the UI. Prefer passing configuration and control state through shared types and command boundaries rather than reaching directly across crates.
- The current audio architecture centers on an instrument/output network, an input or excitement analysis network, per-node controls, telemetry, and quality gating. Use the audio READMEs above as the source of truth instead of duplicating their details here.
- Layout and configuration are screen-dependent and still evolving. When changing layout-driven behavior, inspect the current frontend context providers and backend setup flow before inferring product rules from old comments.

## Working Conventions
- Keep instructions and architecture changes high-signal. Link to detailed docs instead of copying them into new customization files.
- When touching styling, reuse the existing theme and dark-mode strategy from [styles.css](styles.css) and the Leptos app shell.
- When touching DSP or native audio runtime code, check the relevant skill and the crate READMEs first; real-time safety and graph update strategy matter more here than in typical application code.
- Treat generated constants as generated. If behavior comes from build-time constant generation, inspect [crates/common/build.rs](crates/common/build.rs) rather than editing generated outputs.

## Use These Skills
- Use [tailwind-leptos](.github/skills/tailwind-leptos/SKILL.md) for frontend styling work.
- Use [tauri-trunk](.github/skills/tauri-trunk/SKILL.md) for frontend build and hosting changes.
- Use [cpal-runtime](.github/skills/cpal-runtime/SKILL.md) for native audio I/O work.
- Use [fundsp-dsp-design](.github/skills/fundsp-dsp-design/SKILL.md) for DSP graph design and review.

## Good Defaults For Agents
- Prefer reading the backend and audio READMEs before making non-trivial changes.
- Prefer minimal, local edits over broad refactors in DSP or runtime code unless the task clearly requires architectural change.
- If a behavior description in chat conflicts with code, trust the current code and docs, and call out the mismatch explicitly.