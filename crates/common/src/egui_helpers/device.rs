use crate::device::DeviceData;

pub fn device_key(device: &DeviceData) -> String {
    format!("{}::{}", device.host_id, device.device_id)
}

pub fn selected_device_label(device: Option<&DeviceData>) -> String {
    device
        .map(ToString::to_string)
        .unwrap_or_else(|| "Default device".to_string())
}

pub fn find_selected_device<'a>(
    devices: &'a [DeviceData],
    selected_key: Option<&str>,
) -> Option<&'a DeviceData> {
    let key = selected_key?;
    devices.iter().find(|device| device_key(device) == key)
}

pub fn show_device_combo(
    ui: &mut egui::Ui,
    label: &str,
    id_salt: impl std::hash::Hash,
    devices: &[DeviceData],
    selected_key: &mut Option<String>,
    include_device: impl Fn(&DeviceData) -> bool,
) {
    let selected = find_selected_device(devices, selected_key.as_deref());
    ui.label(label);
    egui::ComboBox::from_id_salt(id_salt)
        .selected_text(selected_device_label(selected))
        .show_ui(ui, |ui| {
            for device in devices.iter().filter(|device| include_device(device)) {
                ui.selectable_value(selected_key, Some(device_key(device)), device.to_string());
            }
        });
}
