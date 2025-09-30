mod commands;
mod gates;
mod navigation_manager;

pub use commands::*;
#[allow(unused_imports)]
use shared::{
    error::{NavigationError, Result},
    RouteId,
};
use tauri::{App, Manager};

pub fn setup(app: &mut App) -> Result<()> {
    let nav_manager =
        navigation_manager::NavigationManager::new(app.handle().clone(), RouteId::Home);
    app.manage(nav_manager);
    Ok(())
}
