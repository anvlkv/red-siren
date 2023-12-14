use std::sync::mpsc::{channel, sync_channel, Receiver, Sender, SyncSender};
use std::sync::{Arc, Mutex};

use anyhow::anyhow;
use futures::channel::mpsc::UnboundedSender;
use lazy_static::lazy_static;
use oboe::{
    AudioInputCallback, AudioInputStreamSafe, AudioOutputCallback, AudioOutputStream,
    AudioOutputStreamSafe, AudioStream, AudioStreamAsync, AudioStreamBuilder, AudioStreamSafe,
    ContentType, DataCallbackResult, Input, InputPreset, IsFrameType, Mono, Output,
    PerformanceMode, SharingMode, Stereo, StreamState, Usage,
};
use shared::play::{PlayOperation, PlayOperationOutput};

use crate::{RedSirenAUCapabilities, ViewModel};

type Core = crate::Core<crate::Effect, crate::RedSirenAU>;

lazy_static! {
    // TODO: while this seem to work oboe-rs advises against using a mutex inside audio callback.
    // consider how else to implement the full duplex, which would accept events from app
    // https://github.com/katyo/oboe-rs/issues/56
    static ref CORE: Arc<Mutex<Option<AAUCore>>> = Arc::new(Mutex::new(None));
    static ref OUT_STREAM: Arc<Mutex<Option<AudioStreamAsync<Output, AAUReceiver>>>> =
        Arc::new(Mutex::new(None));
    static ref IN_STREAM: Arc<Mutex<Option<AudioStreamAsync<Input, AUCoreMtx>>>> =
        Arc::new(Mutex::new(None));
}

pub struct AAUCore {
    core: Core,
    render_sender: SyncSender<ViewModel>,
    resolve_sender: Sender<PlayOperationOutput>,
}

impl AAUCore {
    fn update(
        &mut self,
        event: PlayOperation,
        mut rx: Option<UnboundedSender<PlayOperationOutput>>,
    ) {
        let effects = self.core.process_event(event);

        for effect in effects {
            self.process_effect(effect, &mut rx);
        }
    }

    fn process_effect(
        &mut self,
        effect: crate::Effect,
        rx: &mut Option<UnboundedSender<PlayOperationOutput>>,
    ) {
        match effect {
            crate::Effect::Render(_) => self
                .render_sender
                .send(self.core.view())
                .expect("send render"),
            crate::Effect::Resolve(output) => {
                if let Some(rx) = rx.take() {
                    rx.unbounded_send(output.operation).expect("send resolve");
                } else {
                    self.resolve_sender
                        .send(output.operation)
                        .expect("send resolve")
                }
            }
        }
    }
}

struct AUCoreMtx(Arc<Mutex<Option<AAUCore>>>);

impl AudioInputCallback for AUCoreMtx {
    type FrameType = (f32, Mono);

    fn on_audio_ready(
        &mut self,
        _: &mut dyn AudioInputStreamSafe,
        frames: &[<Self::FrameType as IsFrameType>::Type],
    ) -> DataCallbackResult {
        let mut aau = self.0.lock().unwrap_or_else(|poisoned| {
            log::error!("poison in AudioInputCallback: {}", poisoned);
            poisoned.into_inner()
        });
        let aau = aau.as_mut().expect("core");

        aau.update(PlayOperation::Input(vec![Vec::from(frames)]), None);
        DataCallbackResult::Continue
    }
}

struct AAUReceiver(Receiver<ViewModel>);

impl AudioOutputCallback for AAUReceiver {
    type FrameType = (f32, Stereo);

    fn on_audio_ready(
        &mut self,
        _: &mut dyn AudioOutputStreamSafe,
        frames: &mut [(f32, f32)],
    ) -> DataCallbackResult {
        match self.0.recv() {
            Ok(vm) => {
                let ch1 = vm.0.get(0).expect("ch1");
                let ch2 = vm.0.get(1).unwrap_or(ch1);

                for (frame, vm) in frames.iter_mut().zip(ch1.iter().zip(ch2)) {
                    *frame = (*vm.0, *vm.1);
                }

                log::trace!("render frames: {frames:?}");

                DataCallbackResult::Continue
            }
            Err(e) => {
                log::error!("receiver error {e:?}");

                DataCallbackResult::Stop
            }
        }
    }
}

pub struct CoreStreamer;

impl Default for CoreStreamer {
    fn default() -> Self {
        let (render_sender, render_receiver) = sync_channel(1);
        let (resolve_sender, _resolve_receiver) = channel();
        let core = AAUCore {
            core: Core::new::<RedSirenAUCapabilities>(),
            render_sender,
            resolve_sender,
        };

        _ = CORE.lock().expect("core lock").insert(core);

        Self::create_new_input();

        Self::create_new_output(render_receiver);

        Self
    }
}
impl CoreStreamer {
    fn start(&self) -> anyhow::Result<()> {
        let mut stream = IN_STREAM.lock().expect("already busy");
        let stream = stream.as_mut().ok_or(anyhow!("no stream"))?;

        match stream.get_state() {
            StreamState::Open => {
                stream.start()?;
            }
            StreamState::Disconnected => {
                return Err(anyhow!("input stream gone"));
            }
            _ => {}
        };

        let mut stream = OUT_STREAM.lock().expect("already busy");
        let stream = stream.as_mut().ok_or(anyhow!("no stream"))?;

        stream.start()?;

        log::info!("starting");

        Ok(())
    }

    fn pause(&self) -> anyhow::Result<()> {
        let mut stream = OUT_STREAM.lock().unwrap_or_else(|poisoned| {
            log::error!("poison in pause: {}", poisoned);
            poisoned.into_inner()
        });

        let stream = stream.as_mut();
        let stream = stream.ok_or(anyhow!("no stream"))?;

        stream.pause()?;

        // let mut stream = match IN_STREAM.lock() {
        //     Ok(s) => s,
        //     Err(poisoned) => {
        //         log::error!("poison in pause: {}", poisoned);
        //         poisoned.into_inner()
        //     }
        // };
        //
        // let stream = stream.as_mut();
        // let stream = stream.ok_or(anyhow!("no stream"))?;
        //
        // stream.stop()?;

        self.forward(PlayOperation::Suspend, None);

        log::info!("pausing");

        Ok(())
    }

    fn forward(&self, event: PlayOperation, rx: Option<UnboundedSender<PlayOperationOutput>>) {
        let mut aau = CORE.lock().unwrap_or_else(|poisoned| {
            log::error!("poison in forwarding: {}", poisoned);
            poisoned.into_inner()
        });
        let aau = aau.as_mut().expect("core");

        aau.update(event, rx);

        log::info!("forwarding");
    }

    fn create_new_input() {
        let in_stream = AudioStreamBuilder::default()
            .set_performance_mode(PerformanceMode::LowLatency)
            .set_format::<f32>()
            .set_channel_count::<Mono>()
            .set_direction::<Input>()
            .set_input_preset(InputPreset::Unprocessed)
            .set_frames_per_callback(128)
            .set_sample_rate(44100)
            .set_callback(AUCoreMtx(CORE.clone()))
            .open_stream()
            .expect("create input stream");

        _ = IN_STREAM.lock().expect("stream lock").insert(in_stream);
    }

    fn create_new_output(render_receiver: Receiver<ViewModel>) {
        let out_stream = AudioStreamBuilder::default()
            .set_performance_mode(PerformanceMode::LowLatency)
            .set_sharing_mode(SharingMode::Shared)
            .set_format::<f32>()
            .set_channel_count::<Stereo>()
            .set_frames_per_callback(128)
            .set_usage(Usage::Game)
            .set_content_type(ContentType::Music)
            .set_sample_rate(44100)
            .set_callback(AAUReceiver(render_receiver))
            .open_stream()
            .expect("create output stream");

        _ = OUT_STREAM.lock().expect("stream lock").insert(out_stream);
    }
}
impl super::StreamerUnit for CoreStreamer {
    fn update(&self, event: PlayOperation, rx: UnboundedSender<PlayOperationOutput>) {
        match &event {
            PlayOperation::Resume => match self.start() {
                Ok(_) => {
                    log::info!("playing");
                    rx.unbounded_send(PlayOperationOutput::Success(true))
                        .expect("receiver is gone");
                }
                Err(e) => {
                    log::error!("resume error {e:?}");
                    rx.unbounded_send(PlayOperationOutput::Success(false))
                        .expect("receiver is gone");
                }
            },
            PlayOperation::Suspend => match self.pause() {
                Ok(_) => {
                    log::info!("paused");
                    rx.unbounded_send(PlayOperationOutput::Success(true))
                        .expect("receiver is gone");
                }
                Err(e) => {
                    log::error!("suspend error {e:?}");
                    rx.unbounded_send(PlayOperationOutput::Success(false))
                        .expect("receiver is gone");
                }
            },
            _ => self.forward(event, Some(rx)),
        }
    }
}
