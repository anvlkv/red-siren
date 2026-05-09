---
name: leptos-use-reactivity
description: 'Apply leptos-use composables for browser APIs, timing, sizing, and reactive utilities with SSR-safe patterns and feature-aware usage in this workspace.'
---

# Leptos-Use Reactivity

## When to Use
- Replacing custom browser API glue code with `leptos-use` composables.
- Adding reactive timing, resize/measurement, or input/interaction helpers.
- Handling motion/theme/media preferences via composables instead of manual listeners.
- Reviewing whether a feature should use existing enabled `leptos-use` flags or add new ones.

## Procedure
1. Check enabled `leptos-use` features in [crates/frontend/Cargo.toml](../../../crates/frontend/Cargo.toml) before writing code. Prefer already-enabled composables first.
2. Select the smallest composable that matches the job:
   - viewport/layout: `use_window_size`, `use_element_size`, `use_element_bounding`
   - interaction/input: `use_draggable`, `use_mouse_in_element`
   - preferences: `use_preferred_dark`, `use_prefers_reduced_motion`
   - timing: `use_raf_fn`, `use_timeout_fn`, debounced/throttled signal utilities
3. Keep composables close to the component state they drive. Avoid globalizing composables unless multiple independent routes/pages truly share the behavior.
4. Preserve SSR safety assumptions. For target-dependent APIs, prefer `use_window()` or `use_document()` wrappers and other SSR-safe patterns described by leptos-use docs.
5. Use `use_supported` when integrating browser APIs with uneven support. Gate UI behavior reactively instead of relying on user-agent checks.
6. Keep derived state reactive: map composable signals into memos/derived closures consumed directly by `view!`, rather than mutating DOM state manually.
7. If new composables are needed, add minimal feature flags in [crates/frontend/Cargo.toml](../../../crates/frontend/Cargo.toml) and avoid enabling large unrelated feature sets.
8. Prefer `leptos-use` watch/signal helpers for rate limiting (`watch_debounced`, `watch_throttled`, `signal_debounced`) instead of ad-hoc timer code.
9. Validate behavior across viewport sizes, reduced-motion preference states, and interaction edge cases.
10. Confirm cleanup/lifecycle behavior by ensuring listeners and timers are bound through composables rather than manually managed global handlers.

## Completion Checks
- The chosen composable matches the problem domain and removes boilerplate.
- Usage aligns with enabled feature flags in frontend Cargo dependencies.
- SSR-sensitive browser access uses documented safe patterns.
- Reactive outputs flow through signals/memos without manual DOM mutation.
- Timing/listener behavior is composable-based and lifecycle-safe.

## References
- [leptos-use guide](https://leptos-use.rs/get_started.html)
- [leptos-use functions index](https://leptos-use.rs/functions.html)
- [leptos-use SSR guidance](https://leptos-use.rs/server_side_rendering.html)
- [leptos-use crate docs](https://docs.rs/leptos-use/latest/leptos_use/)
- [use_supported docs](https://docs.rs/leptos-use/latest/leptos_use/fn.use_supported.html)
- [crates/frontend/Cargo.toml](../../../crates/frontend/Cargo.toml)
