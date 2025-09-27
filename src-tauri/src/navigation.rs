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
    route: RouteId,
) -> Result<(), String> {
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
    route: RouteId
) -> Result<(), String> {
    let current = manager.current_route();
    if current != route {
        manager.emit_sync_snapshot();
        log::info!(
            "navigation_sync mismatch: route='{route}' backend='{current}' -> emitted snapshot",
        );
    } else {
        log::info!("navigation_sync aligned: '{route}', no emit");
    }
    Ok(())
}
