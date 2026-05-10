use objc2::rc::Retained;
use objc2_app_kit::{
    NSAppearanceCustomization, NSAppearanceNameAqua, NSAppearanceNameDarkAqua, NSView,
};
use objc2_app_kit::{NSColor, NSWindow};
use objc2_foundation::NSArray;
use raw_window_handle::HasWindowHandle;
use tauri::WebviewWindow;

pub fn setup(window: &mut WebviewWindow) -> Result<(), String> {
    if let Ok(handle) = window.window_handle() {
        if let raw_window_handle::RawWindowHandle::AppKit(appkit_handle) = handle.as_raw() {
            let ns_view_ptr = appkit_handle.ns_view.as_ptr();

            unsafe {
                let ns_view: Retained<NSView> = Retained::retain(ns_view_ptr.cast())
                    .ok_or_else(|| String::from("failed to retain NSView"))?;

                let ns_window = ns_view
                    .window()
                    .ok_or_else(|| String::from("NSView was not installed in a window"))?;

                let is_dark_mode = detect_dark_mode(&ns_window);

                let background_color = if is_dark_mode {
                    NSColor::colorWithSRGBRed_green_blue_alpha(
                        0x35 as f64 / 255.0,
                        0x38 as f64 / 255.0,
                        0x39 as f64 / 255.0,
                        1.0,
                    )
                } else {
                    NSColor::colorWithSRGBRed_green_blue_alpha(
                        0xe3 as f64 / 255.0,
                        0x00 as f64 / 255.0,
                        0x22 as f64 / 255.0,
                        1.0,
                    )
                };

                ns_window.setBackgroundColor(Some(&background_color));
                ns_window.setTitlebarAppearsTransparent(true);
                ns_window.setTitleVisibility(objc2_app_kit::NSWindowTitleVisibility::Hidden);
            }
        }
    }

    Ok(())
}

unsafe fn detect_dark_mode(ns_window: &NSWindow) -> bool {
    let effective_appearance = ns_window.effectiveAppearance();
    let appearances = NSArray::from_slice(&[NSAppearanceNameAqua, NSAppearanceNameDarkAqua]);
    let best_match = effective_appearance.bestMatchFromAppearancesWithNames(&appearances);

    match best_match {
        Some(name) => name.isEqualToString(NSAppearanceNameDarkAqua),
        None => false,
    }
}
