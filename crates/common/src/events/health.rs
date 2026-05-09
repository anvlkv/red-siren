pub const SETUP_STATE: &str = "health_setup_state";

pub const NOTIFICATION: &str = "notification";

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationPayload {
    pub content: String,
    pub category: u16,
    pub dismiss: bool,
}
