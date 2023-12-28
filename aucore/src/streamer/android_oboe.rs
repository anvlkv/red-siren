use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};

use anyhow::anyhow;
use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender};
use lazy_static::lazy_static;
use oboe::{
    AudioInputCallback, AudioInputStreamSafe, AudioOutputCallback, AudioOutputStream,
    AudioOutputStreamSafe, AudioStream, AudioStreamAsync, AudioStreamBuilder, AudioStreamSafe,
    ContentType, DataCallbackResult, Error, Input, InputPreset, IsFrameType, Mono, Output,
    PerformanceMode, SharingMode, Stereo, StreamState, Usage,
};

use shared::play::{PlayOperation, PlayOperationOutput};

use crate::{RedSirenAUCapabilities, ViewModel, system::SAMPLE_RATE};

use super::{Core, CoreStreamer};

lazy_static! {
    // TODO: while this seem to work oboe-rs advises against using a mutex inside audio callback.
    // consider how else to implement the full duplex, which would accept events from app
    // https://github.com/katyo/oboe-rs/issues/56
    static ref CORE: Arc<Mutex<Core>> =
        Arc::new(Mutex::new(Core::new::<RedSirenAUCapabilities>()));
    static ref OUT_STREAM: Arc<Mutex<Option<AudioStreamAsync<Output, AAUOutput >>>> =
        Arc::new(Mutex::new(None));
    static ref IN_STREAM: Arc<Mutex<Option<AudioStreamAsync<Input, AAUInput>>>> =
        Arc::new(Mutex::new(None));
}

struct AAUInput {
    render_sender: Sender<ViewModel>,
    op_receiver: Receiver<(PlayOperation, UnboundedSender<PlayOperationOutput>)>,
    resolve_sender: UnboundedSender<PlayOperationOutput>,
}
impl AudioInputCallback for AAUInput {
    type FrameType = (f32, Mono);

    fn on_error_before_close(
        &mut self,
        _audio_stream: &mut dyn AudioInputStreamSafe,
        error: Error,
    ) {
        log::error!("{error:?}");
        self.resolve_sender
            .unbounded_send(PlayOperationOutput::Success(false))
            .expect("send error");
    }

    fn on_error_after_close(&mut self, _audio_stream: &mut dyn AudioInputStreamSafe, error: Error) {
        log::error!("{error:?}");
    }

    fn on_audio_ready(
        &mut self,
        _: &mut dyn AudioInputStreamSafe,
        frames: &[<Self::FrameType as IsFrameType>::Type],
    ) -> DataCallbackResult {
        let core = CORE.lock().expect("input core lock");

        let input: Vec<Vec<f32>> = vec![Vec::from(frames)];
        let mut ops = vec![(PlayOperation::Input(input), self.resolve_sender.clone())];

        match self.op_receiver.try_recv() {
            Ok(op) => {
                ops.push(op);
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                log::error!("op receiver disconnected");
                return DataCallbackResult::Stop;
            }
        };

        for (op, resolve) in ops {
            for effect in core.process_event(op) {
                match effect {
                    crate::Effect::Render(_) => {
                        let view = core.view();
                        self.render_sender.send(view).expect("send render");
                    }
                    crate::Effect::Resolve(op) => {
                        resolve.unbounded_send(op.operation).expect("send resolve");
                    }
                }
            }
        }

        DataCallbackResult::Continue
    }
}

struct AAUOutput(Receiver<ViewModel>, UnboundedSender<PlayOperationOutput>);

impl AudioOutputCallback for AAUOutput {
    type FrameType = (f32, Stereo);

    fn on_error_before_close(
        &mut self,
        _audio_stream: &mut dyn AudioOutputStreamSafe,
        error: Error,
    ) {
        log::error!("{error:?}");
        self.1
            .unbounded_send(PlayOperationOutput::Success(false))
            .expect("send error");
    }

    fn on_error_after_close(
        &mut self,
        _audio_stream: &mut dyn AudioOutputStreamSafe,
        error: Error,
    ) {
        log::error!("{error:?}");
    }

    fn on_audio_ready(
        &mut self,
        _: &mut dyn AudioOutputStreamSafe,
        frames: &mut [(f32, f32)],
    ) -> DataCallbackResult {
        match self.0.recv() {
            Ok(vm) => {
                let ch1 = vm.0.first();
                let ch1 = ch1.iter().flat_map(|v| *v).into_iter();
                let ch2 = vm.0.get(1).or_else(|| vm.0.first());
                let ch2 = ch2.iter().flat_map(|v| *v).into_iter();

                for (frame, vm) in frames.iter_mut().zip(ch1.zip(ch2)) {
                    *frame = (*vm.0, *vm.1);
                }

                DataCallbackResult::Continue
            }
            Err(_) => {
                log::error!("receiver error");
                DataCallbackResult::Stop
            }
        }
    }
}

impl super::StreamerUnit for CoreStreamer {
    fn init(&self) -> anyhow::Result<UnboundedReceiver<PlayOperationOutput>> {
        let (render_sender, render_receiver) = channel::<ViewModel>();
        let (op_sender, op_receiver) = channel();
        let (resolve_sender, resolve_receiver) = futures::channel::mpsc::unbounded();

        let in_stream = AudioStreamBuilder::default()
            .set_performance_mode(PerformanceMode::LowLatency)
            .set_format::<f32>()
            .set_channel_count::<Mono>()
            .set_direction::<Input>()
            .set_input_preset(InputPreset::Unprocessed)
            .set_frames_per_callback(256)
            .set_sample_rate(SAMPLE_RATE as i32)
            .set_callback(AAUInput {
                resolve_sender: resolve_sender.clone(),
                render_sender: render_sender.clone(),
                op_receiver,
            })
            .open_stream()
            .expect("create input stream");

        let out_stream = AudioStreamBuilder::default()
            .set_performance_mode(PerformanceMode::LowLatency)
            .set_sharing_mode(SharingMode::Shared)
            .set_format::<f32>()
            .set_channel_count::<Stereo>()
            .set_frames_per_callback(256)
            .set_usage(Usage::Game)
            .set_content_type(ContentType::Music)
            .set_sample_rate(SAMPLE_RATE as i32)
            .set_callback(AAUOutput(render_receiver, resolve_sender))
            .open_stream()
            .expect("create output stream");

        _ = IN_STREAM.lock().expect("stream lock").insert(in_stream);

        _ = OUT_STREAM.lock().expect("stream lock").insert(out_stream);

        _ = self
            .op_sender
            .lock()
            .expect("op sender lock")
            .insert(op_sender);
        _ = self
            .render_sender
            .lock()
            .expect("render sender lock")
            .insert(render_sender);

        Ok(resolve_receiver)
    }

    fn pause(&self) -> anyhow::Result<()> {
        let mut stream = OUT_STREAM.lock().unwrap_or_else(|poisoned| {
            log::error!("poison in pause: {}", poisoned);
            poisoned.into_inner()
        });

        let stream = stream.as_mut();
        let stream = stream.ok_or(anyhow!("no stream"))?;

        stream.pause()?;

        log::info!("pausing");

        Ok(())
    }

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

    fn forward(
        &self,
        event: PlayOperation,
        resolve_id_sender: UnboundedSender<PlayOperationOutput>,
    ) {
        self.forward_op(CORE.clone(), event, resolve_id_sender);
    }
}
