#[tauri::command]
pub fn health_on_gui_ready() {
    log::info!("GUI ready!");
}
