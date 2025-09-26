/*!
Phase 2 (navigation refactor integrated):
This module now serves as the public backend-facing facade for navigation, delegating
real orchestration to `NavigationManager` (see `navigation_manager.rs`).

WHY (evolution from Phase 1):
- Replaces ad-hoc `NavigationState` with async, cancellable transactions.
- Adds gating (async) + event emission lifecycle.
- Maintains small surface: parse + gating policy + helper constructor.

PUBLIC RESPONSIBILITIES:
- Provide a stable place other backend code can call to:
  * Parse incoming route payloads (mixed legacy/path/enum JSON forms)
  * Instantiate a new `NavigationManager`
  * House the gating policy hook (`can_navigate`) used by the manager

INTENTIONALLY NOT EXPORTED HERE (Phase 2 keeps internal):
- Internal phases enum
- Transaction struct internals

EXTENSION (Phase 3+):
- Introduce leave/enter async hooks
- Timeouts / metrics
- Rich error telemetry

MAYA DRY KISS:
Facade stays minimal; deeper complexity isolated in `navigation_manager.rs`.
*/

use crate::navigation_manager::NavigationManager;
use shared::RouteId;

use tauri::State;

/// Construct a new navigation manager with the given initial route.
/// Keeping creation here localizes future initialization changes (metrics, spans, etc.).
pub fn new_manager(app: tauri::AppHandle, initial: RouteId) -> NavigationManager {
    NavigationManager::new(app, initial)
}

/// Backend gating policy stub.
/// Return `true` to allow, `false` to soft-deny (emits gated allowed=false + canceled),
/// or evolve into richer logic using external state. For *hard* internal errors,
/// adjust the manager's gating wrapper (it currently treats non-deny errors distinctly).
pub fn can_navigate(_to: RouteId) -> bool {
    true
}

/// Accepts mixed legacy / new payloads:
/// - Raw path: "/"
/// - Variant name: "Home"
/// - JSON: "\"Home\"" or {"route":"Home"} or {"path":"/"}
pub fn parse_incoming_route_payload(raw: &str) -> Option<RouteId> {
    shared::navigation::routes::parse_route_id(raw)
}

/// UI signals that the leave animation has completed; proceed to commit.
#[tauri::command]
pub async fn navigation_leave_done(
    manager: State<'_, crate::navigation_manager::NavigationManager>,
    tx_id: u64,
) -> Result<(), String> {
    manager.leave_done(tx_id);
    Ok(())
}

/// UI signals that the enter animation has completed; finalize and complete.
#[tauri::command]
pub async fn navigation_enter_done(
    manager: State<'_, crate::navigation_manager::NavigationManager>,
    tx_id: u64,
) -> Result<(), String> {
    manager.enter_done(tx_id);
    Ok(())
}

#[tauri::command]
pub async fn navigation_request(
    manager: State<'_, NavigationManager>,
    path: String,
) -> Result<(), String> {
    let Some(route) = parse_incoming_route_payload(&path) else {
        log::error!("Invalid route: {}", path);
        return Err("invalid_route".into());
    };
    // Same-route requests are ignored (not an error).
    if manager.request(route).is_none() {
        log::warn!("Same-route requests are ignored (not an error): {route}");
        return Ok(());
    }
    log::info!("navigation_request accepted to={}", route);
    Ok(())
}

/// UI asks backend to emit a one-shot navigation snapshot (used after UI reloads).
#[tauri::command]
pub async fn navigation_sync(
    _app: tauri::AppHandle,
    manager: State<'_, NavigationManager>,
    path: String
) -> Result<(), String> {
    let current = manager.current_route();
    let current_path = current.path();
    if current_path != path {
        manager.emit_sync_snapshot();
        log::info!(
            "navigation_sync mismatch: path='{}' backend='{}' -> emitted snapshot",
            path,
            current_path
        );
    } else {
        log::info!("navigation_sync aligned: '{}', no emit", path);
    }
    Ok(())
}
