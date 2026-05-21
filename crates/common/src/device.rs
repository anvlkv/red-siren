use std::fmt::Display;

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

impl Display for DeviceData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}{} ({})",
            self.device_name,
            if let Some(manufacturer) = &self.device_manufacturer {
                format!(" ({manufacturer})")
            } else {
                String::new()
            },
            self.host_id
        )
    }
}
