mod commands;
mod gates;
mod navigation_manager;

pub use commands::*;
use shared::RouteId;
use tauri::{App, Manager};

pub fn setup(app: &mut App) -> Result<(), String> {
    let nav_manager =
        navigation_manager::NavigationManager::new(app.handle().clone(), RouteId::Home);
    app.manage(nav_manager);
    Ok(())
}
