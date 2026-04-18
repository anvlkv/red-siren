---
name: tauri-trunk
description: 'Configure and maintain Tauri v2 with Trunk for Rust/WASM frontend builds, dev server wiring, and cross-platform frontend hosting.'
---

# Tauri + Trunk

## When to Use
- Wiring or changing the frontend build pipeline for the Tauri app.
- Debugging dev-server, `frontendDist`, hot-reload, or mobile frontend hosting issues.
- Updating Trunk hooks, watch rules, ports, or Tauri build settings.

## Procedure
1. Treat Tauri as a static frontend host. Use Trunk to build or serve the frontend assets and avoid SSR-style assumptions in the app architecture.
2. Check [src-tauri/tauri.conf.json](../../../src-tauri/tauri.conf.json) first. Confirm that `beforeDevCommand`, `beforeBuildCommand`, `devUrl`, `frontendDist`, and `app.withGlobalTauri` match the intended Trunk workflow.
3. Check [Trunk.toml](../../../Trunk.toml) next. Align the served port with Tauri `devUrl`, keep watch ignores focused on non-frontend trees, and preserve any build hooks that copy required assets.
4. When mobile live reload matters, ensure Trunk websocket configuration matches Tauri guidance. The current workspace already pins a custom port; if hot reload fails on mobile, review whether `serve.ws_protocol = "ws"` is needed.
5. Keep build outputs deterministic. If a frontend asset must exist in the Trunk staging directory or dist output, encode that in a Trunk hook instead of relying on manual copying.
6. When adjusting commands, prefer `trunk serve` for development and `trunk build --release` for release builds so the Tauri build pipeline and CI remain aligned.
7. Finish by checking that desktop and mobile assumptions still hold: the dev server must be reachable during development, and the built dist directory must contain all required static assets for packaged builds.

## Completion Checks
- Tauri build settings and Trunk settings agree on port and dist output.
- Required static assets are copied through Trunk hooks or included in dist.
- The workflow remains SPA/SSG-style rather than SSR-dependent.
- Mobile development considerations are called out when websocket reload behavior matters.

## References
- [Tauri frontend configuration](https://v2.tauri.app/start/frontend/)
- [Tauri Trunk guide](https://v2.tauri.app/start/frontend/trunk/)
- [Trunk README](https://github.com/trunk-rs/trunk/blob/main/README.md)
- [src-tauri/tauri.conf.json](../../../src-tauri/tauri.conf.json)
- [Trunk.toml](../../../Trunk.toml)