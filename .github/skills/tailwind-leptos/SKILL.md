---
name: tailwind-leptos
description: 'Style Leptos UI with Tailwind CSS, reactive classes, theme tokens, and Trunk-friendly utility usage for this workspace.'
---

# Tailwind + Leptos

## When to Use
- Adding or refactoring UI in Leptos components.
- Introducing Tailwind utilities, theme tokens, dark mode, or component styling patterns.
- Reviewing whether styling should live in Tailwind utilities, shared CSS, or Leptos reactive attributes.

## Procedure
1. Inspect the existing theme and global styling first in [styles.css](../../../styles.css) and the target component under [crates/frontend/src](../../../crates/frontend/src). Reuse the current palette, font, and dark-mode approach instead of inventing a parallel styling system.
2. Keep Tailwind utility classes literal inside `view!` markup so Tailwind can discover them reliably during builds. Do not depend on runtime-generated utility class names.
3. Use Leptos reactive attributes for UI state changes: prefer `class:name=...`, `class=(..., ...)`, `style:name=...`, and derived signals when a single class or style needs to react to state.
4. Put reusable design tokens and global rules in [styles.css](../../../styles.css): theme colors, fonts, custom variants, keyframes, scrollbar rules, and shared CSS variables belong there.
5. Keep component markup readable. Use Tailwind utilities for layout, spacing, typography, and one-off visual treatment; move repeated or high-noise patterns into shared CSS classes or variables.
6. Preserve the existing dark-mode strategy. This workspace already defines a custom dark variant in CSS and toggles theme classes in the Leptos app shell, so new UI should plug into that model instead of adding a new one.
7. Before finishing, verify that the styling works across desktop and mobile-sized layouts and that utility classes remain consistent with the workspace visual language.

## Completion Checks
- New classes follow the current palette and typography.
- Reactive styling uses Leptos idioms instead of manual DOM mutation.
- Tailwind class names remain statically discoverable.
- Shared tokens live in the global stylesheet, not duplicated across components.

## References
- [Tailwind CSS CLI installation](https://tailwindcss.com/docs/installation/tailwind-cli)
- [Leptos dynamic classes, styles, and attributes](https://book.leptos.dev/view/02_dynamic_attributes.html)
- [styles.css](../../../styles.css)
- [crates/frontend/src/app.rs](../../../crates/frontend/src/app.rs)