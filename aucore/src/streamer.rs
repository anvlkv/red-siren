use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, TryLockError};

use anyhow::Result;
use futures::channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use futures::executor::ThreadPool;
use futures::task::SpawnExt;
use futures::StreamExt;

use shared::play::{PlayOperation, PlayOperationOutput};

use crate::ViewModel;

#[cfg_attr(not(any(feature = "android", feature = "ios")), allow(dead_code))]
pub type Core = crate::Core<crate::Effect, crate::RedSirenAU>;

pub trait StreamerUnit {
    fn init(&self) -> Result<UnboundedReceiver<PlayOperationOutput>>;
    fn pause(&self) -> Result<()>;
    fn start(&self) -> Result<()>;
    fn forward(
        &self,
        event: PlayOperation,
        resolve_id_sender: UnboundedSender<PlayOperationOutput>,
    );
}

pub type OPSender = Sender<(PlayOperation, UnboundedSender<PlayOperationOutput>)>;
#[derive(Default)]
#[cfg_attr(not(any(feature = "android", feature = "ios")), allow(dead_code))]
struct CoreStreamer {
    op_sender: Arc<Mutex<Option<OPSender>>>,
    render_sender: Arc<Mutex<Option<Sender<ViewModel>>>>,
}

#[cfg_attr(not(any(feature = "android", feature = "ios")), allow(dead_code))]
impl CoreStreamer {
    fn forward_op(
        &self,
        core: Arc<Mutex<Core>>,
        event: PlayOperation,
        resolve_id_sender: UnboundedSender<PlayOperationOutput>,
    ) {
        let op_sender = self.op_sender.clone();
        let op_sender = op_sender.lock().expect("lock op sender");
        let render_sender = self.render_sender.clone();

        let render_sender = render_sender.lock().expect("lock render sender");

        match core.try_lock() {
            Err(TryLockError::WouldBlock) => {
                if let Some(sender) = op_sender.as_ref() {
                    sender.send((event, resolve_id_sender)).expect("send op");
                } else {
                    log::warn!("no sender, core blocked");
                }
            }
            Ok(core) => {
                if let Some(sender) = render_sender.as_ref() {
                    for effect in core.process_event(event) {
                        match effect {
                            crate::Effect::Render(_) => {
                                sender.send(core.view()).expect("send render");
                            }
                            crate::Effect::Resolve(output) => resolve_id_sender
                                .unbounded_send(output.operation)
                                .expect("send output"),
                        }
                    }
                } else {
                    log::warn!("no sender, core");
                }
            }
            Err(TryLockError::Poisoned(e)) => {
                log::error!("poisoned {e:?}");
                resolve_id_sender
                    .unbounded_send(PlayOperationOutput::Success(false))
                    .expect("send error");
            }
        }
    }
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

impl Default for AUCoreBridge {
    fn default() -> Self {
        Self::new()
    }
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

        log::debug!("{event:?}");
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

        assert_eq!(
            outs.len(),
            1,
            "expected exactly one output for play operation"
        );

        outs.pop().unwrap()
    }
}
