use shared::RouteId;
use tauri::State;

use super::navigation_manager::NavigationManager;


/// Bootstrap initial navigation transaction (tx_id=0).
/// Invoke once from the UI after event listeners are mounted so the
/// initial Home page appear animation can complete the transaction.
/// Idempotent: subsequent calls short‑circuit and return 0.
#[tauri::command]
pub async fn navigation_bootstrap(
    manager: State<'_, NavigationManager>,
) -> Result<(), String> {
    _ = manager.bootstrap_initial_transaction();
    Ok(())
}

/// UI signals that the leave animation has completed; proceed to commit.
#[tauri::command]
pub async fn navigation_leave_done(
    manager: State<'_, NavigationManager>,
    tx_id: u64,
) -> Result<(), String> {
    manager.leave_done(tx_id);
    Ok(())
}

/// UI signals that the enter animation has completed; finalize and complete.
#[tauri::command]
pub async fn navigation_enter_done(
    manager: State<'_, NavigationManager>,
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
    route: RouteId,
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


#[tauri::command]
pub fn navigation_resume(manager: State<'_, NavigationManager>,) {
    if let Some(tx) = manager.resume_pending() {
        log::info!("resume pending navigation, new tx_id={tx}");
    }
    else {
        log::warn!("no pending navigation to resume...")
    }
}
