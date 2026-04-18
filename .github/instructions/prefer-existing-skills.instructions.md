---
applyTo: "**"
---

# Prefer Existing Skills

When a task clearly falls into one of the workspace's established areas, load and follow the matching skill before making changes.

- Use [.github/skills/tailwind-leptos/SKILL.md](../skills/tailwind-leptos/SKILL.md) when changing Leptos UI, Tailwind classes, theme tokens, dark mode behavior, or shared frontend styling patterns.
- Use [.github/skills/tauri-trunk/SKILL.md](../skills/tauri-trunk/SKILL.md) when changing Tauri frontend hosting, Trunk config, dev/build commands, ports, static asset copying, or desktop/mobile frontend build wiring.
- Use [.github/skills/cpal-runtime/SKILL.md](../skills/cpal-runtime/SKILL.md) when changing native audio I/O, CPAL stream setup, device selection, sample-format handling, or callback behavior.
- Use [.github/skills/fundsp-dsp-design/SKILL.md](../skills/fundsp-dsp-design/SKILL.md) when changing DSP graphs, synthesis, filters, modulation, signal routing, parameter smoothing, or other FunDSP structure.

If a task spans multiple concerns, load all relevant skills before proceeding. Typical overlaps:
- DSP plus native runtime: use both `fundsp-dsp-design` and `cpal-runtime`.
- Frontend UI plus build/hosting: use both `tailwind-leptos` and `tauri-trunk`.

Do not force these skills onto unrelated work. Use them when the task meaningfully touches their area, not just because a file happens to live nearby.