use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};

use anyhow::Result;
use futures::channel::mpsc::{unbounded, UnboundedSender};
use futures::executor::ThreadPool;
use futures::task::SpawnExt;
use futures::StreamExt;

use lazy_static::lazy_static;
use app_core::play::{PlayOperation, PlayOperationOutput};

pub use futures::channel::mpsc::UnboundedReceiver;

use crate::{Effect, RedSirenAUCapabilities, ViewModel};

#[cfg_attr(not(any(feature = "android", feature = "ios")), allow(dead_code))]
pub type Core = crate::Core<crate::Effect, crate::RedSirenAU>;

lazy_static! {
    static ref CORE: Arc<Mutex<Core>> = Arc::new(Mutex::new(Core::new::<RedSirenAUCapabilities>()));
}

pub trait StreamerUnit {
    fn init(&self) -> Result<()>;
    fn pause(&self) -> Result<()>;
    fn start(&self) -> Result<()>;
}

#[derive(Clone)]
#[cfg_attr(not(any(feature = "android", feature = "ios")), allow(dead_code))]
struct CoreStreamer {
    pub op_receiver: Arc<Mutex<Receiver<PlayOperation>>>,
    pub op_sender: Arc<Mutex<Sender<PlayOperation>>>,
    pub resolve_sender: Arc<Mutex<UnboundedSender<PlayOperationOutput>>>,
    pub render_sender: Arc<Mutex<Sender<ViewModel>>>,
    pub render_receiver: Arc<Mutex<Receiver<ViewModel>>>,
    pub input_sender: Arc<Mutex<Sender<Vec<Vec<f32>>>>>,
    pub input_receiver: Arc<Mutex<Receiver<Vec<Vec<f32>>>>>,
}

#[cfg_attr(not(any(feature = "android", feature = "ios")), allow(dead_code))]
impl CoreStreamer {
    fn new() -> (Self, UnboundedReceiver<PlayOperationOutput>) {
        let (render_sender, render_receiver) = channel::<ViewModel>();
        let (op_sender, op_receiver) = channel::<PlayOperation>();
        let (input_sender, input_receiver) = channel::<Vec<Vec<f32>>>();
        let (resolve_sender, resolve_receiver) = unbounded::<PlayOperationOutput>();

        (
            Self {
                render_sender: Arc::new(Mutex::new(render_sender)),
                render_receiver: Arc::new(Mutex::new(render_receiver)),
                resolve_sender: Arc::new(Mutex::new(resolve_sender)),
                op_sender: Arc::new(Mutex::new(op_sender)),
                op_receiver: Arc::new(Mutex::new(op_receiver)),
                input_sender: Arc::new(Mutex::new(input_sender)),
                input_receiver: Arc::new(Mutex::new(input_receiver)),
            },
            resolve_receiver,
        )
    }

    fn forward(
        &self,
        event: PlayOperation,
        resolve_id_sender: UnboundedSender<PlayOperationOutput>,
    ) {
        let op_sender = self.op_sender.lock().expect("lock op sender");

        let mut resolve = self.resolve_sender.lock().expect("lock resolve");
        *resolve = resolve_id_sender;

        op_sender.send(event).expect("send op");
    }
}

cfg_if::cfg_if! {
    if #[cfg(feature="android")]{
        mod android_oboe;
    } 
    else if #[cfg(feature="ios")] {
        mod ios_coreaudio;
    } 
    else {
        impl StreamerUnit for CoreStreamer {
            fn init(&self) -> Result<()> {
                unimplemented!()
            }
            fn pause(&self) -> Result<()> {
                unimplemented!()
            }
            fn start(&self) -> Result<()> {
                unimplemented!()
            }
        }
    }
}

pub struct AUCoreBridge {
    core: Arc<Mutex<CoreStreamer>>,
    pool: ThreadPool,
}

impl Default for AUCoreBridge {
    fn default() -> Self {
        Self::new()
    }
}

impl AUCoreBridge {
    pub fn new() -> Self {
        let pool = ThreadPool::new().expect("create a thread pool for updates");
        let (core_streamer, _) = CoreStreamer::new();
        let CoreStreamer {
            op_receiver,
            resolve_sender,
            render_sender,
            input_receiver,
            ..
        } = core_streamer.clone();

        let core = CORE.clone();
        pool.spawn(async move {
            let input_receiver = input_receiver.lock().expect("input receiver lock");

            while let Ok(input) = input_receiver.recv() {
                let core = core.lock().expect("tick core lock");
                let render_sender = render_sender.lock().expect("render lock");
                let op_receiver = op_receiver.lock().expect("op receiver lock");
                let resolve_sender = resolve_sender.lock().expect("resolve sender lock");
                let mut ops = vec![PlayOperation::Input(input)];

                match op_receiver.try_recv() {
                    Ok(op) => {
                        ops.push(op);
                    }
                    Err(TryRecvError::Empty) => {}
                    Err(e) => {
                        log::error!("op recv error: {e:?}");
                    }
                };

                log::trace!("processor process events");

                for op in ops {
                    for effect in core.process_event(op) {
                        match effect {
                            Effect::Render(_) => {
                                let view = core.view();
                                render_sender.send(view).expect("send render");
                            }
                            Effect::Resolve(op) => resolve_sender
                                .unbounded_send(op.operation)
                                .expect("send resolve"),
                            Effect::Capture(d) => {
                                todo!()
                            }
                        }
                    }
                }

                log::trace!("processor tick");
            }

            log::debug!("processor job exited");
        })
        .expect("process handle");

        AUCoreBridge {
            pool,
            core: Arc::new(Mutex::new(core_streamer)),
        }
    }

    pub fn request(&self, bytes: Vec<u8>) -> UnboundedReceiver<Vec<u8>> {
        let (s_id, mut r_id) = unbounded::<PlayOperationOutput>();

        let event =
            bincode::deserialize::<PlayOperation>(bytes.as_slice()).expect("deserialize op");

        log::trace!("request {event:?}");
        
        let core = self.core.clone();

        let tx_bridge = async move {
            let core = core.lock().expect("lock core");
            match &event {
                PlayOperation::InstallAU => match core.init() {
                    Ok(_) => {
                        log::info!("init au");
                        s_id.unbounded_send(PlayOperationOutput::Success)
                            .expect("receiver is gone");
                    }
                    Err(e) => {
                        log::error!("resume error {e:?}");
                        s_id.unbounded_send(PlayOperationOutput::Failure)
                            .expect("receiver is gone");
                    }
                },
                PlayOperation::Resume => match core.start() {
                    Ok(_) => {
                        log::info!("playing");
                        s_id.unbounded_send(PlayOperationOutput::Success)
                            .expect("receiver is gone");
                    }
                    Err(e) => {
                        log::error!("resume error {e:?}");
                        s_id.unbounded_send(PlayOperationOutput::Failure)
                            .expect("receiver is gone");
                    }
                },
                PlayOperation::Suspend => match core.pause() {
                    Ok(_) => {
                        log::info!("paused");
                    }
                    Err(e) => {
                        log::error!("suspend error {e:?}");
                    }
                },
                _ => core.forward(event, s_id),
            }
        };

        self.pool.spawn(tx_bridge).expect("spawn bridge");

        let (sx, rx) = unbounded();

        let cx_future = async move {
            while let Some(d) = r_id.next().await {
                log::trace!("send play op output");
                sx.unbounded_send(bincode::serialize(&d).expect("serialize output"))
                    .expect("send msg");
            }
            log::debug!("request receive complete");
        };

        self.pool.spawn(cx_future).expect("spawn convert");

        rx
    }
}
