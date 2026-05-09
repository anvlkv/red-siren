---
description: "Use when changing Leptos frontend source files. Select the right frontend skill for styling, router architecture, and leptos-use reactive utilities in crates/frontend/src."
applyTo: "crates/frontend/src/**"
---

# Frontend Skill Selection

When changing files under [crates/frontend/src](../../../crates/frontend/src), load the frontend skill that matches the behavior being changed instead of relying on a generic Leptos workflow.

- Use [tailwind-leptos](../skills/tailwind-leptos/SKILL.md) for Tailwind classes, theme tokens, layout styling, typography, dark mode behavior, and reactive class/style binding.
- Use [leptos-router-architecture](../skills/leptos-router-architecture/SKILL.md) for changes to [crates/frontend/src/app.rs](../../../crates/frontend/src/app.rs), [crates/frontend/src/routes.rs](../../../crates/frontend/src/routes.rs), route trees, `Router`, `Routes`, `ParentRoute`, `Outlet`, params, queries, links, or navigation flow.
- Use [leptos-use-reactivity](../skills/leptos-use-reactivity/SKILL.md) for `leptos-use` hooks, browser API wrappers, viewport or measurement utilities, reduced-motion or theme preference detection, timers, and signal debouncing or throttling.

Combine skills when the task spans multiple concerns.

- Styling plus router-driven UI: load `tailwind-leptos` and `leptos-router-architecture`.
- Styling plus browser-reactive utilities: load `tailwind-leptos` and `leptos-use-reactivity`.
- Router state plus browser/reactive utilities: load `leptos-router-architecture` and `leptos-use-reactivity`.

Do not treat every frontend file as a router task. Choose the narrowest matching skill set for the behavior being edited.