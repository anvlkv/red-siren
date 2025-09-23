#[tauri::command]
pub fn on_gui_ready() {
    log::info!("GUI ready!");
}
