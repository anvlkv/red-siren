use std::sync::mpsc::TryRecvError;
use std::sync::{Arc, Mutex};

use anyhow::anyhow;
use lazy_static::lazy_static;
use oboe::{
    AudioInputCallback, AudioInputStreamSafe, AudioOutputCallback, AudioOutputStream,
    AudioOutputStreamSafe, AudioStream, AudioStreamAsync, AudioStreamBuilder, AudioStreamSafe,
    ContentType, DataCallbackResult, Error, Input, InputPreset, IsFrameType, Mono, Output,
    PerformanceMode, SharingMode, Stereo, StreamState, Usage,
};

use app_core::play::PlayOperationOutput;

use crate::system::SAMPLE_RATE;

use super::CoreStreamer;

lazy_static! {
    static ref OUT_STREAM: Arc<Mutex<Option<AudioStreamAsync<Output, CoreStreamer>>>> =
        Arc::new(Mutex::new(None));
    static ref IN_STREAM: Arc<Mutex<Option<AudioStreamAsync<Input, CoreStreamer>>>> =
        Arc::new(Mutex::new(None));
}
impl AudioInputCallback for CoreStreamer {
    type FrameType = (f32, Mono);

    fn on_error_before_close(
        &mut self,
        _audio_stream: &mut dyn AudioInputStreamSafe,
        error: Error,
    ) {
        log::error!("{error:?}");
        let rs = self.resolve_sender
            .lock()
            .expect("lock resolve");

            rs.unbounded_send(PlayOperationOutput::Failure)
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
        let input: Vec<Vec<f32>> = vec![Vec::from(frames)];
        let input_sender = self.input_sender.lock().expect("lock input");
        match input_sender.send(input) {
            Ok(_) => {
                log::trace!("sending input");
                DataCallbackResult::Continue
            },
            Err(e) => {
                log::error!("send input: {e:?}");
                DataCallbackResult::Stop
            }
        }
    }
}

impl AudioOutputCallback for CoreStreamer {
    type FrameType = (f32, Stereo);

    fn on_error_before_close(
        &mut self,
        _audio_stream: &mut dyn AudioOutputStreamSafe,
        error: Error,
    ) {
        log::error!("{error:?}");
        let rs = self
            .resolve_sender
            .lock()
            .expect("resolve lock");

        rs.unbounded_send(PlayOperationOutput::Failure)
            .expect("send error")
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
        let render = self.render_receiver.lock().expect("render lock");
        match render.try_recv() {
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
            Err(TryRecvError::Empty) => DataCallbackResult::Continue,
            Err(e) => {
                log::error!("receiver error: {e:?}");
                DataCallbackResult::Stop
            }
        }
    }
}

impl super::StreamerUnit for CoreStreamer {
    fn init(&self) -> anyhow::Result<()> {
        let in_stream = AudioStreamBuilder::default()
            .set_performance_mode(PerformanceMode::LowLatency)
            .set_format::<f32>()
            .set_channel_count::<Mono>()
            .set_direction::<Input>()
            .set_input_preset(InputPreset::Unprocessed)
            .set_frames_per_callback(256)
            .set_sample_rate(SAMPLE_RATE as i32)
            .set_callback(self.clone())
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
            .set_callback(self.clone())
            .open_stream()
            .expect("create output stream");

        _ = IN_STREAM.lock().expect("stream lock").insert(in_stream);

        _ = OUT_STREAM.lock().expect("stream lock").insert(out_stream);

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
}
