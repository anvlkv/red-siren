use tauri::{WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_window_state::WindowExt;

mod health;

#[cfg(target_os = "macos")]
mod setup_mac_window;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let win_builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::default())
                .title("Red Siren")
                .inner_size(800.0, 600.0);

            // set transparent title bar only when building for macOS
            #[cfg(target_os = "macos")]
            let win_builder = win_builder.title_bar_style(tauri::TitleBarStyle::Transparent);

            let mut window = win_builder.build().unwrap();

            // set background color only when building for macOS
            #[cfg(target_os = "macos")]
            {
                setup_mac_window::setup(&mut window)?;
            }

            window.restore_state(tauri_plugin_window_state::StateFlags::all())?;

            Ok(())
        })
        .plugin(tauri_plugin_window_state::Builder::new().build())
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![health::health_on_gui_ready])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
