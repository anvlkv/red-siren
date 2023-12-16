use std::sync::Arc;

pub fn process_event(data: &[u8]) -> Vec<u8> {
    shared::process_event(data)
}

pub fn handle_response(uuid: &[u8], data: &[u8]) -> Vec<u8> {
    shared::handle_response(uuid, data)
}

pub fn view() -> Vec<u8> {
    shared::view()
}

pub fn log_init() {
    shared::log_init();
    aucore::au_log_init();
}

#[derive(uniffi::Object)]
pub struct AUCoreBridge(aucore::AUCoreBridge);

#[uniffi::export]
pub fn au_new() -> Arc<AUCoreBridge> {
    Arc::new(AUCoreBridge(aucore::AUCoreBridge::new()))
}

#[uniffi::export]
pub async fn au_request(arc_self: Arc<AUCoreBridge>, bytes: Vec<u8>) -> Vec<u8> {
    let au = arc_self.clone();
    au.0.request(bytes).await
}

uniffi::include_scaffolding!("ffirs");
