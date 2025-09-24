use objc2::rc::Retained;
use objc2_app_kit::{
    NSAppearanceCustomization, NSAppearanceNameAqua, NSAppearanceNameDarkAqua, NSView,
};
use objc2_app_kit::{NSColor, NSWindow};
use objc2_foundation::NSArray;
use raw_window_handle::HasWindowHandle;
use tauri::WebviewWindow;

pub fn setup(window: &mut WebviewWindow) -> Result<(), Box<dyn std::error::Error>> {
    if let Ok(handle) = window.window_handle() {
        if let raw_window_handle::RawWindowHandle::AppKit(appkit_handle) = handle.as_raw() {
            let ns_view_ptr = appkit_handle.ns_view.as_ptr();

            unsafe {
                // Get NSView from the pointer

                let ns_view: Retained<NSView> =
                    Retained::retain(ns_view_ptr.cast()).ok_or("Failed to retain NSView")?;

                // Get NSWindow from NSView
                let ns_window = ns_view
                    .window()
                    .ok_or("NSView was not installed in a window")?;

                // Detect if we're in dark mode
                let is_dark_mode = detect_dark_mode(&ns_window)?;

                // Set custom background color based on theme
                let background_color = if is_dark_mode {
                    // Dark mode: use black (#353839)
                    NSColor::colorWithSRGBRed_green_blue_alpha(
                        0x35 as f64 / 255.0, // 0.208
                        0x38 as f64 / 255.0, // 0.220
                        0x39 as f64 / 255.0, // 0.224
                        1.0,
                    )
                } else {
                    // Light mode: use red (#e30022)
                    NSColor::colorWithSRGBRed_green_blue_alpha(
                        0xe3 as f64 / 255.0, // 0.890
                        0x00 as f64 / 255.0, // 0.000
                        0x22 as f64 / 255.0, // 0.133
                        1.0,
                    )
                };

                ns_window.setBackgroundColor(Some(&background_color));

                // Make the titlebar transparent
                ns_window.setTitlebarAppearsTransparent(true);

                // Hide the title
                ns_window.setTitleVisibility(objc2_app_kit::NSWindowTitleVisibility::Hidden);
            }
        }
    }

    Ok(())
}

unsafe fn detect_dark_mode(ns_window: &NSWindow) -> Result<bool, Box<dyn std::error::Error>> {
    // Get the effective appearance of the window
    let effective_appearance = ns_window.effectiveAppearance();

    // Create array of appearance names to check against
    let appearances = NSArray::from_slice(&[NSAppearanceNameAqua, NSAppearanceNameDarkAqua]);
    let best_match = effective_appearance.bestMatchFromAppearancesWithNames(&appearances);

    match best_match {
        Some(name) => Ok(name.isEqualToString(NSAppearanceNameDarkAqua)),
        None => Ok(false), // Default to light mode if we can't determine
    }
}
