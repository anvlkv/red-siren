use anyhow::anyhow;
use core::f32::consts::PI;
use futures::channel::mpsc::UnboundedSender;
use lazy_static::lazy_static;
use oboe::{
    AudioOutputCallback, AudioOutputStream, AudioOutputStreamSafe, AudioStream, AudioStreamAsync,
    AudioStreamBuilder, DataCallbackResult, Output, PerformanceMode, SharingMode, Status, Stereo,
};
use shared::play::{PlayOperation, PlayOperationOutput};
use std::sync::{Arc, Mutex};

type Core = aucore::Core<aucore::Effect, aucore::RedSirenAU>;

pub struct AAUCore {
    core: Core,
    vm: aucore::ViewModel,
    // sine
    frequency: f32,
    gain: f32,
    phase: f32,
    delta: Option<f32>,
}

impl AAUCore {
    fn new() -> Self {
        Self {
            core: aucore::Core::<aucore::Effect, aucore::RedSirenAU>::new::<
                aucore::RedSirenAUCapabilities,
            >(),
            frequency: 440.0,
            gain: 0.5,
            phase: 0.0,
            delta: None,
            vm: Default::default(),
        }
    }

    fn update(
        &mut self,
        event: shared::play::PlayOperation,
        mut rx: Option<UnboundedSender<PlayOperationOutput>>,
    ) {
        let effects = self.core.process_event(event);

        for effect in effects {
            self.process_effect(effect, &mut rx);
        }
    }

    fn process_effect(
        &mut self,
        effect: aucore::Effect,
        rx: &mut Option<UnboundedSender<PlayOperationOutput>>,
    ) {
        match effect {
            aucore::Effect::Render(_) => {
                self.vm = self.core.view();
            }
            aucore::Effect::Resolve(output) => {
                if let Some(rx) = rx.take() {
                    rx.unbounded_send(output.operation).expect("send resolve");
                } else {
                    todo!()
                    // self.s_out.send(output.operation)
                }
            }
        }
    }
}

struct AUCoreMtx(Arc<Mutex<AAUCore>>);

impl AudioOutputCallback for AUCoreMtx {
    // Define type for frames which we would like to process
    type FrameType = (f32, Stereo);

    // Implement sound data output callback
    fn on_audio_ready(
        &mut self,
        stream: &mut dyn AudioOutputStreamSafe,
        frames: &mut [(f32, f32)],
    ) -> DataCallbackResult {
        let mut aau = match self.0.try_lock() {
            Ok(a) => a,
            Err(e) => {
                log::error!("output skips turn {e:?}");
                return DataCallbackResult::Continue;
            }
        };
        log::debug!("playing frame");
        // Configure out wave generator
        if aau.delta.is_none() {
            let sample_rate = stream.get_sample_rate() as f32;
            aau.delta = (aau.frequency * 2.0 * PI / sample_rate).into();
            log::info!(
                "Prepare sine wave generator: samplerate={}, time delta={}",
                sample_rate,
                aau.delta.unwrap()
            );
        }

        let delta = aau.delta.unwrap();

        // Generate audio frames to fill the output buffer
        for frame in frames.iter_mut() {
            *frame = (
                aau.gain * aau.phase.sin(),
                aau.gain * aau.phase.sin() * -1.0,
            );
            aau.phase += delta;
            while aau.phase > 2.0 * PI {
                aau.phase -= 2.0 * PI;
            }
        }

        log::debug!("frame: {frames:?}");

        // Notify the oboe that stream is continued
        DataCallbackResult::Continue
    }
}

lazy_static! {
    static ref CORE: Arc<Mutex<AAUCore>> = Arc::new(Mutex::new(AAUCore::new()));
    static ref OUT_STREAM: Arc<Mutex<Option<AudioStreamAsync<Output, AUCoreMtx>>>> =
        Arc::new(Mutex::new(None));
}

pub struct CoreStreamer;

impl Default for CoreStreamer {
    fn default() -> Self {
        let stream = AudioStreamBuilder::default()
            // select desired performance mode
            .set_performance_mode(PerformanceMode::LowLatency)
            // select desired sharing mode
            .set_sharing_mode(SharingMode::Shared)
            // select sound sample format
            .set_format::<f32>()
            // select channels configuration
            .set_channel_count::<Stereo>()
            // set our generator as callback
            .set_callback(AUCoreMtx(CORE.clone()))
            // open the output stream
            .open_stream()
            .expect("create stream");

        _ = OUT_STREAM.lock().expect("stream lock").insert(stream);

        Self
    }
}
impl CoreStreamer {
    fn start(&self) -> anyhow::Result<()> {
        let mut stream = OUT_STREAM.lock().expect("already busy");

        let stream = stream.as_mut().ok_or(anyhow!("no stream"))?;
        stream.start()?;

        Ok(())
    }

    fn pause(&self) -> anyhow::Result<()> {
        match OUT_STREAM.try_lock() {
            Ok(mut l) => {
                let stream = l.as_mut();
                let stream = stream.ok_or(anyhow!("no stream"))?;

                stream.pause()?;

                Ok(())
            }
            Err(_) => {
                std::thread::sleep(std::time::Duration::from_micros(20));
                self.pause()
            }
        }
    }

    fn forward(&self, event: PlayOperation, rx: UnboundedSender<PlayOperationOutput>) {
        match CORE.try_lock() {
            Ok(mut core) => core.update(event, Some(rx)),
            Err(_) => {
                std::thread::sleep(std::time::Duration::from_micros(20));
                return self.forward(event, rx);
            }
        }
    }

    pub fn update(&self, event: PlayOperation, rx: UnboundedSender<PlayOperationOutput>) {
        match &event {
            PlayOperation::Resume => match self.start() {
                Ok(_) => {
                    log::info!("playing");
                    rx.unbounded_send(PlayOperationOutput::Success(true))
                        .expect("receiver is gone");
                }
                Err(e) => {
                    log::error!("{e:?}");
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
                    log::error!("{e:?}");
                    rx.unbounded_send(PlayOperationOutput::Success(false))
                        .expect("receiver is gone");
                }
            },
            _ => self.forward(event, rx),
        }
    }
}
