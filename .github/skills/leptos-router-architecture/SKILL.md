---
name: leptos-router-architecture
description: 'Design and refactor Leptos Router structure: route trees, nested layouts with Outlet, params/query handling, and navigation patterns for this workspace.'
---

# Leptos Router Architecture

## When to Use
- Adding or refactoring route trees in Leptos frontend code.
- Introducing nested route layouts with `ParentRoute` and `Outlet`.
- Implementing route params/query parsing and URL-driven state.
- Reviewing navigation behavior (`A`, `use_navigate`) and route-shell composition.

## Procedure
1. Start from route ownership boundaries in [crates/frontend/src/app.rs](../../../crates/frontend/src/app.rs) and [crates/frontend/src/routes.rs](../../../crates/frontend/src/routes.rs). Keep one top-level `Router` and place shared app chrome outside `Routes`.
2. Model route hierarchy by layout responsibility, not string deduplication. Use `ParentRoute` only when parent and child views should render together on screen.
3. For each nested route tree, ensure parent views include an `Outlet` where child routes should render. Missing `Outlet` is a common cause of hidden child views.
4. Prefer URL-driven state for major navigation state. If the state should survive refresh/share/back-forward, model it as path params or query params.
5. Choose typed or map-style params/query deliberately:
   - Use `use_params` and `use_query` (typed) when URL fields have stable schema and should parse into domain types.
   - Use `use_params_map` and `use_query_map` when schema is dynamic, optional, or exploratory.
6. Handle parse and absence cases explicitly. Typed hooks return `Memo<Result<T, _>>`; derive fallback behavior for invalid or missing values instead of assuming success.
7. Use `use_navigate` for imperative transitions triggered by events/effects, and plain route links (`A`) for static navigation in markup.
8. Keep route components reactive and lightweight: derive view state from router memos/signals, avoid manual URL parsing, and avoid duplicate local state that shadows route state.
9. When extracting route modules, keep route definitions composable and localized. Preserve readability by grouping related nested routes near their layout component.
10. Validate expected behavior manually for direct-link load, refresh on deep routes, fallback/not-found rendering, and browser back-forward transitions.

## Completion Checks
- A single top-level `Router` controls global navigation state.
- Nested routes that should render children include `Outlet` in the parent view.
- Params and query values are read via Leptos Router hooks, not handwritten URL parsing.
- Parse failures and missing URL values have explicit fallback behavior.
- Route structure matches layout behavior and supports deep links/back-forward navigation.

## References
- [Leptos Router: Defining Routes](https://book.leptos.dev/router/16_routes.html)
- [Leptos Router: Nested Routing](https://book.leptos.dev/router/17_nested_routing.html)
- [Leptos Router: Params and Queries](https://book.leptos.dev/router/18_params_and_queries.html)
- [leptos_router hooks on docs.rs](https://docs.rs/leptos_router/latest/leptos_router/hooks/index.html)
- [use_navigate docs](https://docs.rs/leptos_router/latest/leptos_router/hooks/fn.use_navigate.html)
- [crates/frontend/src/app.rs](../../../crates/frontend/src/app.rs)
- [crates/frontend/src/routes.rs](../../../crates/frontend/src/routes.rs)
