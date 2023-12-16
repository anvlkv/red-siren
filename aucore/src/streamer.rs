use std::sync::{Arc, Mutex};

use futures::channel::mpsc::unbounded;
use futures::executor::ThreadPool;
use futures::task::SpawnExt;
use futures::StreamExt;

pub trait StreamerUnit
where
    Self: Default,
{
    fn update(
        &self,
        _: shared::play::PlayOperation,
        _: futures::channel::mpsc::UnboundedSender<shared::play::PlayOperationOutput>,
    );
}

// TODO: if there's a way to build uniffi with oboe-rs...
cfg_if::cfg_if! {
    if #[cfg(feature="android")]{
        mod android_oboe;
        use android_oboe::CoreStreamer;
    } else if #[cfg(feature="ios")] {
        mod ios_coreaudio;
        use ios_coreaudio::CoreStreamer;
    } else {

        #[derive(Default)]
        struct CoreStreamer;

        impl StreamerUnit for CoreStreamer {
            fn update(&self, _: shared::play::PlayOperation, _: futures::channel::mpsc::UnboundedSender<shared::play::PlayOperationOutput>){
                unreachable!("not implemented")
            }
        }
    }
}

pub struct AUCoreBridge {
    core: Arc<Mutex<CoreStreamer>>,
    // r_out: Arc<Mutex<Receiver<PlayOperationOutput>>>,
    pool: ThreadPool,
}

pub fn new() -> AUCoreBridge {
    // let (s_out, r_out) = channel::<shared::play::PlayOperationOutput>();
    let pool = ThreadPool::new().expect("create a thread pool for updates");
    AUCoreBridge {
        #[allow(clippy::default_constructed_unit_structs)]
        core: Arc::new(Mutex::new(CoreStreamer::default())),
        // r_out: Arc::new(Mutex::new(r_out)),
        pool,
    }
}

pub async fn request(arc_self: &AUCoreBridge, bytes: Vec<u8>) -> Vec<u8> {
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
