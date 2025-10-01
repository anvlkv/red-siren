/// Window appearance (dark mode or light mode) changed
pub const UPDATE_WINDOW_APPEARANCE: &str = "update_window_appearance";
/// Window size (width and height) changed
pub const UPDATE_WINDOW_SIZE: &str = "update_window_size";
/// Safe area insets applied to instrument layout, originating from ui elements
pub const UI_SAFE_AREA_INSETS_INCREMENT: &str = "ui_safe_area_insets_increment";

#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SafeAreaInstestUiIncrementPayload {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}
