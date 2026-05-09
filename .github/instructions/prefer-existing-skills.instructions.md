---
applyTo: "**"
---

# Prefer Existing Skills

When a task clearly falls into one of the workspace's established areas, load and follow the matching skill before making changes.

- Use [.github/skills/tailwind-leptos/SKILL.md](../skills/tailwind-leptos/SKILL.md) when changing Leptos UI, Tailwind classes, theme tokens, dark mode behavior, or shared frontend styling patterns.
- Use [.github/skills/leptos-router-architecture/SKILL.md](../skills/leptos-router-architecture/SKILL.md) when changing Leptos Router trees, nested routes, Outlet placement, URL params/query handling, or navigation behavior.
- Use [.github/skills/leptos-use-reactivity/SKILL.md](../skills/leptos-use-reactivity/SKILL.md) when changing `leptos-use` hooks, browser API composables, timing/listener utilities, SSR-safe browser access, or reactive utility wiring.
- Use [.github/skills/tauri-trunk/SKILL.md](../skills/tauri-trunk/SKILL.md) when changing Tauri frontend hosting, Trunk config, dev/build commands, ports, static asset copying, or desktop/mobile frontend build wiring.
- Use [.github/skills/cpal-runtime/SKILL.md](../skills/cpal-runtime/SKILL.md) when changing native audio I/O, CPAL stream setup, device selection, sample-format handling, or callback behavior.
- Use [.github/skills/fundsp-dsp-design/SKILL.md](../skills/fundsp-dsp-design/SKILL.md) when changing DSP graphs, synthesis, filters, modulation, signal routing, parameter smoothing, or other FunDSP structure.
- Use [.github/skills/num-rational-docs-first/SKILL.md](../skills/num-rational-docs-first/SKILL.md) when using or reviewing `num-rational` APIs, including constructor invariants, checked arithmetic, float conversions, parsing/formatting, and feature-flag choices.

If a task spans multiple concerns, load all relevant skills before proceeding. Typical overlaps:
- DSP plus native runtime: use both `fundsp-dsp-design` and `cpal-runtime`.
- Frontend styling plus route behavior: use `tailwind-leptos` and `leptos-router-architecture`.
- Frontend styling plus browser-reactive utilities: use `tailwind-leptos` and `leptos-use-reactivity`.
- Frontend route behavior plus browser-reactive utilities: use `leptos-router-architecture` and `leptos-use-reactivity`.
- Frontend UI plus build/hosting: use `tailwind-leptos` and `tauri-trunk`.

Do not force these skills onto unrelated work. Use them when the task meaningfully touches their area, not just because a file happens to live nearby.