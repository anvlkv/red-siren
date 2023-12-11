use std::sync::{Arc, Mutex};

use futures::channel::mpsc::unbounded;
use futures::executor::ThreadPool;
use futures::task::SpawnExt;
use futures::StreamExt;

#[cfg(feature = "ndk")]
mod aau;

#[uniffi::export]
pub fn au_log_init() {
    let lvl = log::LevelFilter::Debug;

    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(lvl)
            .with_tag("red_siren::shared"),
    );

    log::debug!("init logging")
}

// TODO: if there's a way to build uniffi with oboe-rs...
cfg_if::cfg_if! {
  if #[cfg(not(feature = "ndk"))] {
        #[derive(Default)]
        struct CoreStreamer;

        impl CoreStreamer {
            pub fn update(&self, _: shared::play::PlayOperation, _: futures::channel::mpsc::UnboundedSender<shared::play::PlayOperationOutput>) {
                unreachable!("ndk feature not enabled")
            }
        }

    }  else {
        use aau::CoreStreamer;
    }
}

#[derive(uniffi::Object)]
pub struct AUCoreBridge {
    core: Arc<Mutex<CoreStreamer>>,
    // r_out: Arc<Mutex<Receiver<PlayOperationOutput>>>,
    pool: ThreadPool,
}

#[uniffi::export]
pub fn new() -> Arc<AUCoreBridge> {
    // let (s_out, r_out) = channel::<shared::play::PlayOperationOutput>();
    let pool = ThreadPool::new().expect("create a thread pool for updates");
    Arc::new(AUCoreBridge {
        #[allow(clippy::default_constructed_unit_structs)]
        core: Arc::new(Mutex::new(CoreStreamer::default())),
        // r_out: Arc::new(Mutex::new(r_out)),
        pool,
    })
}

#[uniffi::export]
pub async fn request(arc_self: Arc<AUCoreBridge>, bytes: Vec<u8>) -> Vec<u8> {
    let (s_id, r_id) = unbounded::<shared::play::PlayOperationOutput>();

    let op = bincode::deserialize::<shared::play::PlayOperation>(bytes.as_slice())
        .expect("deserialize op");
    let core = arc_self.core.clone();
    let tx_result = async move {
        let core = core.lock().expect("streamer locked");
        core.update(op, s_id);
    };

    arc_self.pool.spawn(tx_result).expect("cant spawn task");

    let mut outs = r_id
        .map(|out| bincode::serialize(&out).expect("serialize output"))
        .collect::<Vec<_>>()
        .await;

    assert_eq!(outs.len(), 1);

    outs.pop().unwrap()
}

uniffi::include_scaffolding!("aaucore");
