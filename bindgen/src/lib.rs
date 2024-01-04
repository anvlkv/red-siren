use futures::{lock::Mutex, StreamExt};
use std::sync::Arc;

pub fn process_event(data: &[u8]) -> Vec<u8> {
    app_core::process_event(data)
}

pub fn handle_response(uuid: &[u8], data: &[u8]) -> Vec<u8> {
    app_core::handle_response(uuid, data)
}

pub fn view() -> Vec<u8> {
    app_core::view()
}

pub fn log_init() {
    let lvl = log::LevelFilter::Trace;
    app_core::log_init(lvl);
    aucore::au_log_init(lvl);
}

#[derive(uniffi::Object)]
pub struct AUCoreBridge(aucore::AUCoreBridge);

#[uniffi::export]
pub fn au_new() -> Arc<AUCoreBridge> {
    Arc::new(AUCoreBridge(aucore::AUCoreBridge::new()))
}

#[derive(uniffi::Object)]
pub struct AUReceiver(Mutex<aucore::UnboundedReceiver<Vec<u8>>>);

#[uniffi::export]
pub fn au_request(arc_self: Arc<AUCoreBridge>, bytes: Vec<u8>) -> Arc<AUReceiver> {
    let au = arc_self.clone();
    Arc::new(AUReceiver(Mutex::new(au.0.request(bytes))))
}

#[uniffi::export]
pub async fn au_receive(arc_self: Arc<AUReceiver>) -> Option<Vec<u8>> {
    let mut rx = arc_self.0.lock().await;
    rx.next().await
}

uniffi::include_scaffolding!("ffirs");
