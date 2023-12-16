use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use futures::channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use futures::executor::ThreadPool;
use futures::task::SpawnExt;
use futures::StreamExt;

use shared::play::{PlayOperation, PlayOperationOutput};

use crate::ViewModel;

pub trait StreamerUnit {
    fn init(&self) -> Result<UnboundedReceiver<PlayOperationOutput>>;
    fn pause(&self) -> Result<()>;
    fn start(&self) -> Result<()>;
    fn forward(&self, op: PlayOperation, rx: UnboundedSender<PlayOperationOutput>);
}

#[derive(Default)]
#[cfg_attr(not(any(feature = "android", feature = "ios")), allow(dead_code))]
struct CoreStreamer {
    op_sender: Arc<Mutex<Option<Sender<(PlayOperation, UnboundedSender<PlayOperationOutput>)>>>>,
    render_sender: Arc<Mutex<Option<Sender<ViewModel>>>>,
}

cfg_if::cfg_if! {
    if #[cfg(feature="android")]{
        mod android_oboe;
    } else if #[cfg(feature="ios")] {
        mod ios_coreaudio;
    } else {
        impl StreamerUnit for CoreStreamer {
            fn init(&self) -> Result<UnboundedReceiver<PlayOperationOutput>> {
                unreachable!("no platform feature")
            }
            fn pause(&self) -> Result<()> {
                unreachable!("no platform feature")
            }
            fn start(&self) -> Result<()> {
                unreachable!("no platform feature")
            }

            fn forward(&self, _: PlayOperation, _: UnboundedSender<PlayOperationOutput>) {
                unreachable!("no platform feature")
            }

        }
    }
}

pub struct AUCoreBridge {
    core: Arc<Mutex<CoreStreamer>>,
    pool: ThreadPool,
    resolve_receiver: Arc<Mutex<Option<UnboundedReceiver<PlayOperationOutput>>>>,
}

impl AUCoreBridge {
    pub fn new() -> Self {
        let pool = ThreadPool::new().expect("create a thread pool for updates");

        AUCoreBridge {
            pool,
            core: Default::default(),
            resolve_receiver: Default::default(),
        }
    }

    pub async fn request(&self, bytes: Vec<u8>) -> Vec<u8> {
        let (s_id, r_id) = unbounded::<PlayOperationOutput>();

        let event =
            bincode::deserialize::<PlayOperation>(bytes.as_slice()).expect("deserialize op");

        let core = self.core.clone();
        let resolve_receiver = self.resolve_receiver.clone();

        let tx_result = async move {
            let core = core.lock().expect("lock core");
            match &event {
                PlayOperation::InstallAU => match core.init() {
                    Ok(receiver) => {
                        log::info!("init au");
                        _ = resolve_receiver.lock().unwrap().insert(receiver);
                        s_id.unbounded_send(PlayOperationOutput::Success(true))
                            .expect("receiver is gone");
                    }
                    Err(e) => {
                        log::error!("resume error {e:?}");
                        s_id.unbounded_send(PlayOperationOutput::Success(false))
                            .expect("receiver is gone");
                    }
                },
                PlayOperation::Resume => match core.start() {
                    Ok(_) => {
                        log::info!("playing");
                        s_id.unbounded_send(PlayOperationOutput::Success(true))
                            .expect("receiver is gone");
                    }
                    Err(e) => {
                        log::error!("resume error {e:?}");
                        s_id.unbounded_send(PlayOperationOutput::Success(false))
                            .expect("receiver is gone");
                    }
                },
                PlayOperation::Suspend => match core.pause() {
                    Ok(_) => {
                        log::info!("paused");
                        s_id.unbounded_send(PlayOperationOutput::Success(true))
                            .expect("receiver is gone");
                    }
                    Err(e) => {
                        log::error!("suspend error {e:?}");
                        s_id.unbounded_send(PlayOperationOutput::Success(false))
                            .expect("receiver is gone");
                    }
                },
                _ => core.forward(event, s_id),
            }
        };

        self.pool.spawn(tx_result).expect("cant spawn task");

        let mut outs = r_id
            .map(|out| bincode::serialize(&out).expect("serialize output"))
            .collect::<Vec<_>>()
            .await;

        assert_eq!(outs.len(), 1);

        outs.pop().unwrap()
    }
}
