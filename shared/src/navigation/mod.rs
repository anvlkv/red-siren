/*!
WHY:
Phase 1 navigation module root. Exposes typed route identifiers (`RouteId`)
so frontend (Leptos) and backend (Tauri) share a consistent representation.
This is the minimal scaffold; orchestration/state machine arrives in Phase 2.

STRUCTURE:
- `routes.rs`: defines `RouteId`, path mapping, and tolerant parser
- This `mod.rs`: re-exports `RouteId` and documents intent

EXTENSION:
Add new enum variants in `routes.rs` and update its match arms.

POLICY (documented for future phases):
- Same-route navigation requests will be ignored (handled by manager later).
- Backend gating will introduce a `can_navigate(RouteId)` function (stub to be added in Tauri code).

MAYA DRY KISS: Keep surface tiny & explicit.
*/

pub mod routes;

pub use routes::RouteId;
