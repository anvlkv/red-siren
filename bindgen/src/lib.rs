use futures::{lock::Mutex, StreamExt};
use std::sync::Arc;

pub fn process_event(data: &[u8]) -> Vec<u8> {
    cfg_if::cfg_if! {
        if #[cfg(not(feature="browser"))]  {
            app_core::process_event(data)
        }
        else {
            unimplemented!()
        }
    }
}

pub fn handle_response(uuid: &[u8], data: &[u8]) -> Vec<u8> {
    cfg_if::cfg_if! {
        if #[cfg(not(feature="browser"))]  {
            app_core::handle_response(uuid, data)
        }
        else {
            unimplemented!()
        }
    }
}

pub fn view() -> Vec<u8> {
    cfg_if::cfg_if! {
        if #[cfg(not(feature="browser"))]  {
            app_core::view()
        }
        else {
            unimplemented!()
        }
    }
}

pub fn log_init() {
    let lvl = log::LevelFilter::Warn;
    
    app_core::log_init(lvl);
    aucore::log_init(lvl);
}

#[derive(uniffi::Object)]
pub struct AUCoreBridge(aucore::AUCoreBridge);

impl AUCoreBridge {
    pub fn au_new() -> Self {
        AUCoreBridge(aucore::AUCoreBridge::new())
    }
}

#[uniffi::export]
pub fn au_new() -> Arc<AUCoreBridge> {
    Arc::new(AUCoreBridge::au_new())
}

#[derive(uniffi::Object)]
pub struct AUReceiver(Mutex<aucore::UnboundedReceiver<Vec<u8>>>);

impl AUReceiver {
    pub fn au_request(au: &AUCoreBridge, bytes: Vec<u8>) -> AUReceiver {
        AUReceiver(Mutex::new(au.0.request(bytes)))
    }

    async fn au_receive(&self) -> Option<Vec<u8>> {
        log::trace!("wait for receiver lock");
        let mut rx = self.0.lock().await;
        log::trace!("wait for sender message");
        rx.next()
            .await
    }
}

#[uniffi::export]
pub fn au_request(arc_self: Arc<AUCoreBridge>, bytes: Vec<u8>) -> Arc<AUReceiver> {
    let au = arc_self.clone();
    Arc::new(AUReceiver::au_request(au.as_ref(), bytes))
}

#[uniffi::export]
pub async fn au_receive(arc_self: Arc<AUReceiver>) -> Option<Vec<u8>> {
    arc_self.au_receive().await
}

uniffi::include_scaffolding!("ffirs");
