---
name: cpal-runtime
description: 'Design or review CPAL audio I/O code for device selection, stream configuration, callback safety, and cross-platform runtime behavior.'
---

# CPAL Runtime

## When to Use
- Adding or changing native audio I/O.
- Debugging device enumeration, sample-format handling, or stream startup.
- Reviewing real-time safety in audio callbacks.

## Procedure
1. Start from the runtime boundary. Identify whether the change affects host selection, device selection, stream configuration, callback rendering, or stream lifecycle.
2. Choose devices deliberately. Prefer the default host and default input or output device when that matches the product behavior; otherwise enumerate devices and surface a clear selection strategy.
3. Query supported configs before building a stream unless the product has a verified fixed config. Treat device/config queries as fallible because devices can disappear or reject requests.
4. Match the stream callback to the actual sample format and convert explicitly when needed. Do not assume `f32` unless the selected config guarantees it.
5. Keep the callback real-time safe: no blocking, no heavy logging, no file I/O, no allocation-heavy work, and no lock contention that can glitch audio.
6. Keep the stream alive and start it explicitly with `play()`. If the design supports pausing, use `pause()` only outside the real-time path.
7. Model failure paths clearly: no device available, config negotiation failed, stream build failed, runtime stream error, and device disconnects all need explicit behavior.
8. In this workspace, align CPAL work with the native runtime feature boundary in [crates/audio-system/Cargo.toml](../../../crates/audio-system/Cargo.toml), especially the `rt_cpal` feature and any cross-platform fallback runtime story.

## Completion Checks
- Device and config selection are explicit and fallible.
- Sample-format handling matches the chosen config.
- Callback logic is real-time safe.
- Stream lifetime is preserved and startup behavior is explicit.
- Native runtime changes remain consistent with workspace feature flags.

## References
- [CPAL crate docs](https://docs.rs/cpal/latest/cpal/)
- [crates/audio-system/Cargo.toml](../../../crates/audio-system/Cargo.toml)
- [Cargo.toml](../../../Cargo.toml)