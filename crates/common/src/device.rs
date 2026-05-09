use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceData {
    pub host_id: String,
    pub device_id: String,
    pub device_name: String,
    pub device_manufacturer: Option<String>,
    pub supports_input: bool,
    pub supports_output: bool,
}
