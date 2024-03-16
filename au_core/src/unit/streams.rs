use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use anyhow::{anyhow, Result};
use crux_core::capability::{CapabilityContext, Operation};
use crux_macros::Capability;
use fundsp::hacker32::*;
use parking_lot::Mutex;
use ringbuf::HeapProducer;
use send_wrapper::SendWrapper;
use serde::{Deserialize, Serialize};

use crate::{model::UnitState, AppAuBuffer, FFTData, SnoopsData, System, UnitEvent};

const RENDER_BURSTS: usize = 5;

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub struct StreamsOp;

impl Operation for StreamsOp {
    type Output = ();
}

#[derive(Capability)]
pub struct AUStreams<Ev> {
    context: CapabilityContext<StreamsOp, Ev>,
    streams: Arc<
        Mutex<(
            Option<SendWrapper<cpal::Stream>>,
            Option<SendWrapper<cpal::Stream>>,
        )>,
    >,
}

impl<Ev> AUStreams<Ev>
where
    Ev: 'static,
{
    pub fn new(context: CapabilityContext<StreamsOp, Ev>) -> Self {
        Self {
            context,
            streams: Default::default(),
        }
    }

    pub fn run<R>(&self, fft_res: usize, buffer_size: usize, app_au_buffer: AppAuBuffer, on_run: R)
    where
        R: Fn(bool) -> Ev + Send + 'static,
    {
        self.context.spawn({
            let context = self.context.clone();
            let (input_be, render_be) = {
                let (input, output) = (sys.input_net.backend(), sys.output_net.backend());
                (
                    BigBlockAdapter32::new(Box::new(input)),
                    BlockRateAdapter32::new(Box::new(output)),
                )
            };
            cfg_if::cfg_if!( if #[cfg(any(feature = "worklet", feature = "cpal"))]
            {
            });

            cfg_if::cfg_if!(if #[cfg(feature = "browser")] {
                cfg_if::cfg_if!(if #[cfg(feature="worklet")] {
                    let backend = self.backend.clone();
                } else {
                    let on_msg_mtx = self.msg_closure.clone();
                    let node_mtx = self.node.clone();
                    let ctx_mtx = self.ctx.clone();
                })
            });

            cfg_if::cfg_if!(if #[cfg(feature = "cpal")]
            {
                let streams = self.streams.clone();
            });

            async move {
                cfg_if::cfg_if!(if #[cfg(feature ="browser")] {
                    cfg_if::cfg_if!(if #[cfg(feature ="worklet")] {
                        let mut mtx = backend.lock();
                        *mtx = SendWrapper::new(Self::make_backend(
                            buffer_size,
                            fft_res,
                            input_be,
                            render_be,
                            app_au_buffer,
                        ));
                        context.update_app(on_run(true));
                    } else {
                        wasm_bindgen_futures::spawn_local(async move {
                            match Self::run_audio_ctx(app_au_buffer).await {
                                Ok((node, ctx, msg_closure)) => {
                                    _ = node_mtx.lock().insert(SendWrapper::new(node));
                                    _ = ctx_mtx.lock().insert(SendWrapper::new(ctx));
                                    _ = on_msg_mtx.lock().replace(SendWrapper::new(msg_closure));
                                    context.update_app(on_run(true));
                                }
                                Err(e) => {
                                    log::error!("run unit err: {}", e);
                                    context.update_app(on_run(false));
                                }
                            }
                        });
                    })
                });

                #[cfg(feature = "cpal")]
                match Self::run_cpal_streams(
                    buffer_size,
                    fft_res,
                    input_be,
                    render_be,
                    app_au_buffer,
                ) {
                    Ok((output, input)) => {
                        let mut mtx = streams.lock();
                        if let Some(output) = output {
                            _ = mtx.0.insert(SendWrapper::new(output));
                            if let Some(input) = input {
                                _ = mtx.1.insert(SendWrapper::new(input));
                            }
                            context.update_app(on_run(true));
                        } else {
                            log::error!("no output stram");
                            context.update_app(on_run(false));
                        }
                    }
                    Err(e) => {
                        log::error!("run unit err: {}", e);
                        context.update_app(on_run(false));
                    }
                }
            }
        });
    }

    #[cfg(all(feature = "browser", not(feature = "worklet")))]
    pub fn forward(&self, ev: UnitEvent) {
        use js_sys::{Object, Reflect, Uint8Array};
        let node = self.node.lock();
        if let Some(node) = node.as_ref() {
            let port = node.port().unwrap();

            let value = bincode::serialize(&ev).unwrap();
            let arr = Uint8Array::from(value.as_slice());
            let msg = Object::new();
            Reflect::set(&msg, &"type".into(), &"update".into()).unwrap();
            Reflect::set(&msg, &"value".into(), &arr).unwrap();

            port.post_message(&msg).unwrap();
        }
    }

    #[cfg(feature = "worklet")]
    fn make_backend(
        buffer_size: usize,
        fft_res: usize,
        mut input_be: BigBlockAdapter32,
        mut render_be: BlockRateAdapter32,
        app_au_buffer: AppAuBuffer,
    ) -> Box<dyn FnMut(Vec<f32>) -> Vec<f32>> {
        use itertools::interleave;
    }

    #[cfg(feature = "worklet")]
    pub fn process_input(&self, input: Vec<f32>) {}

    fn run_cpal_streams(
        buffer_size: usize,
        fft_res: usize,
        mut input_be: BigBlockAdapter32,
        mut render_be: BlockRateAdapter32,
        app_au_buffer: AppAuBuffer,
    ) -> Result<()> {
        let err_fn = |e: cpal::StreamError| {
            log::error!("stream error: {e}");
        };

        log::debug!("buffer size: {}", buffer_size);
        let rn_rb = HeapRb::<(f32, f32)>::new(buffer_size * RENDER_BURSTS);
        let an_rb = HeapRb::<f32>::new(fft_res * RENDER_BURSTS);

        let mut process_fn = move || -> Result<()> {
            Self::process_input(&mut consume_analyze)?;
            Self::produce_output(&mut produce_render, &mut render_be)?;
            Self::send_snoops_reading();
            Ok(())
        };

        // let config_buffer_size = cpal::BufferSize::Fixed(self.buffer_size);

        Ok((out_stream, in_stream))
    }

    fn store_input(
        input_analyzer_enabled: AtomicBool,
        data: &[f32],
        produce_analyze: &mut HeapProducer<f32>,
        input_be: &mut BigBlockAdapter32,
    ) -> Result<()> {
        let mut output_frames = vec![0.0_f32; data.len()];
        let is_enabled = input_analyzer_enabled.load(Ordering::Acquire);

        if is_enabled {
            input_be.process(data.len(), &[data], &mut [output_frames.as_mut_slice()]);
            let mut it = output_frames.into_iter();
            produce_analyze.push_iter(&mut it);
        }

        Ok(())
    }

    fn produce_output(
        buffer_size: usize,
        produce: &mut HeapProducer<(f32, f32)>,
        render_be: &mut BlockRateAdapter32,
    ) -> Result<()> {
        let overflow = produce.is_full();

        if !overflow {
            for _ in 0..Ord::min(buffer_size, produce.free_len()) {
                produce.push(render_be.get_stereo()).unwrap();
            }
        }

        Ok(())
    }

    pub fn playing<F>(&self, notify: F, start: bool)
    where
        F: Fn(UnitState) -> Ev + Send + 'static,
    {
        self.context.spawn({
            let context = self.context.clone();

            #[cfg(feature = "cpal")]
            let streams = self.streams.clone();
            #[cfg(all(feature = "browser", not(feature = "worklet")))]
            let ctx = self.ctx.clone();

            async move {
                #[cfg(feature = "cpal")]
                {
                    let streams = streams.lock();
                    if let Some(stream) = streams.0.as_ref() {
                        if start {
                            cpal::traits::StreamTrait::pause(stream as &cpal::Stream)
                                .expect("pause output");
                            if let Some(stream) = streams.1.as_ref() {
                                cpal::traits::StreamTrait::pause(stream as &cpal::Stream)
                                    .expect("pause input");
                            }
                            context.update_app(notify(UnitState::Paused));
                            log::info!("stream: Suspended");
                        } else {
                            cpal::traits::StreamTrait::play(stream as &cpal::Stream)
                                .expect("play output");
                            if let Some(stream) = streams.1.as_ref() {
                                cpal::traits::StreamTrait::play(stream as &cpal::Stream)
                                    .expect("play input");
                            }
                            context.update_app(notify(UnitState::Playing));
                            log::info!("stream: Resumed");
                        }
                    } else {
                        context.update_app(notify(UnitState::None));
                        log::error!("no stream")
                    }
                }
                #[cfg(all(feature = "browser", not(feature = "worklet")))]
                {
                    use wasm_bindgen_futures::JsFuture;

                    if let Some(ctx) = ctx.lock().as_ref() {
                        let promise = if start { ctx.resume() } else { ctx.suspend() };
                        wasm_bindgen_futures::spawn_local(async move {
                            match promise.map(|promise| JsFuture::from(promise)) {
                                Ok(fut) => match fut.await {
                                    Ok(_) => {
                                        if start {
                                            context.update_app(notify(UnitState::Playing));
                                        } else {
                                            context.update_app(notify(UnitState::Paused));
                                        }
                                    }
                                    Err(_) => {
                                        context.update_app(notify(UnitState::None));
                                    }
                                },
                                Err(_) => {
                                    context.update_app(notify(UnitState::None));
                                }
                            }
                        });
                    } else {
                        context.update_app(notify(UnitState::None));
                        log::error!("no context");
                    }
                }
            }
        })
    }
}
