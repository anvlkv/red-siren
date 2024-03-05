#[allow(unused)]
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::{Node, System, MAX_F, MIN_F};
use anyhow::{anyhow, Error, Result};
use fundsp::hacker32::*;
#[allow(unused)]
use futures::channel::mpsc::UnboundedSender;
use hecs::Entity;
#[allow(unused)]
use ringbuf::{HeapConsumer, HeapProducer, HeapRb, StaticConsumer, StaticProducer, StaticRb};
use serde::{Deserialize, Serialize};
use spectrum_analyzer::{
    samples_fft_to_spectrum, scaling::divide_by_N_sqrt, windows::hann_window, FrequencyLimit,
};

cfg_if::cfg_if! { if #[cfg(feature="browser")] {
    cfg_if::cfg_if!{ if #[cfg(not(feature="worklet"))]{
                thread_local! {
                    static CTX: Arc<Mutex<Option<web_sys::AudioContext>>> = Default::default();
                    static NODE: Arc<Mutex<Option<web_sys::AudioWorkletNode>>> = Default::default();
                    static ON_MESSAGE: Arc<Mutex<wasm_bindgen::closure::Closure<dyn FnMut(wasm_bindgen::JsValue)>>> = Arc::new(Mutex::new(wasm_bindgen::closure::Closure::wrap(Box::new(|_| {}) as Box<dyn FnMut(wasm_bindgen::JsValue)>)));
                }
            }
        }
    }
    else {
        thread_local! {
            static IN_STREAM: Arc<Mutex<Option<cpal::Stream>>> = Default::default();
            static OUT_STREAM: Arc<Mutex<Option<cpal::Stream>>> = Default::default();
            static POOL: futures::executor::ThreadPool = futures::executor::ThreadPool::new().expect("create pool");
        }
    }
}

#[allow(dead_code)]
const RENDER_BURSTS: usize = 5;
pub const FFT_RES: usize = 1024;

pub const FFT_BUF_SIZE: usize = 4;
pub const SNOOPS_BUF_SIZE: usize = 8;

#[derive(Clone)]
pub struct Unit {
    pub nodes: Arc<Mutex<HashMap<Entity, Node>>>,
    pub sample_rate: u32,
    pub fft_res: usize,
    pub buffer_size: u32,
    pub system: Arc<Mutex<System>>,
    #[cfg(not(feature = "worklet"))]
    resolve_sender: UnboundedSender<UnitResolve>,
    input_analyzer_enabled: Arc<Mutex<bool>>,
    #[cfg(feature = "worklet")]
    consumers: Arc<
        Mutex<
            Option<(
                StaticConsumer<Vec<(f32, f32)>, FFT_BUF_SIZE>,
                StaticConsumer<Vec<(Entity, Vec<f32>)>, SNOOPS_BUF_SIZE>,
            )>,
        >,
    >,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub enum UnitResolve {
    RunUnit(bool),
    UpdateEV(bool),
}

#[derive(Deserialize, Serialize)]
pub enum UnitEV {
    ButtonPressed(Entity),
    ButtonReleased(Entity),
    Detune(Entity, f32),
    Configure(Vec<Node>),
    SetControl(Entity, f32),
    ListenToInput,
    IgnoreInput,
    Suspend,
    Resume,
}

#[allow(unused)]
impl Unit {
    pub fn new(
        #[cfg(not(feature = "worklet"))] resolve_sender: UnboundedSender<UnitResolve>,
    ) -> Self {
        cfg_if::cfg_if! {if #[cfg(feature = "browser")] {
            let (sample_rate, buffer_size) = (44100, 128);
        } else {
            let (sample_rate, buffer_size) = cpal::traits::HostTrait::default_output_device(&cpal::default_host())
            .map(|d| cpal::traits::DeviceTrait::default_output_config(&d).ok())
            .flatten()
            .map(|d| {
                (
                    d.sample_rate().0,
                    match d.buffer_size() {
                        cpal::SupportedBufferSize::Range { min: _, max } => *max,
                        cpal::SupportedBufferSize::Unknown => 1024,
                    },
                )
            })
            .unwrap_or((44100, 1024));
        }}

        let fft_res = FFT_RES;

        let sys = System::new(sample_rate);

        let nodes = sys.nodes.clone();
        let system = Arc::new(Mutex::new(sys));

        Self {
            fft_res,
            system,
            nodes,
            sample_rate,
            buffer_size,
            input_analyzer_enabled: Arc::new(Mutex::new(true)),
            #[cfg(not(feature = "worklet"))]
            resolve_sender,
            #[cfg(feature = "worklet")]
            consumers: Default::default(),
        }
    }

    pub fn update(&mut self, ev: UnitEV) {
        cfg_if::cfg_if! {
            if #[cfg(all(feature = "browser", not(feature = "worklet")))] {
                match ev {
                    UnitEV::Resume => {
                        CTX.with(|mtx| {
                            if let Ok(mtx) = mtx.lock() {
                                if let Some(ctx) = mtx.as_ref() {
                                    _ = ctx.resume().expect("play ctx");
                                }
                            }
                        });
                    }
                    UnitEV::Suspend => {
                        CTX.with(|mtx| {
                            if let Ok(mtx) = mtx.lock() {
                                if let Some(ctx) = mtx.as_ref() {
                                    _ = ctx.suspend().expect("play ctx");
                                }
                            }
                        });
                    }
                    ev => {
                        use js_sys::{Object, Reflect, Uint8Array};
                        NODE.with(|mtx| {
                            let mut mtx = mtx.lock().unwrap();
                            let node = mtx.as_mut().unwrap();

                            let port = node.port().unwrap();

                            let value = bincode::serialize(&ev).unwrap();
                            let arr = Uint8Array::from(value.as_slice());
                            let msg = Object::new();
                            Reflect::set(&msg, &"type".into(), &"update".into()).unwrap();
                            Reflect::set(&msg, &"value".into(), &arr).unwrap();

                            port.post_message(&msg).unwrap();
                        });
                    }
                }
            }
            else {
                match ev {
                    UnitEV::ButtonPressed(e) => {
                        let sys = self.system.lock().expect("system lock");
                        sys.press(e, true)
                    }
                    UnitEV::ButtonReleased(e) => {
                        let sys = self.system.lock().expect("system lock");
                        sys.press(e, false)
                    }
                    UnitEV::Detune(e, val) => {
                        let sys = self.system.lock().expect("system lock");
                        sys.move_f(e, val)
                    }
                    UnitEV::SetControl(e, val) => {
                        let nodes = self.nodes.lock().expect("lock nodes");
                        if let Some(node) = nodes.get(&e) {
                            log::info!("set control val {val}");
                            node.control.set_value(val);
                        } else {
                            log::error!("no node for entity")
                        }
                    }
                    UnitEV::Configure(nodes) => {
                        let mut sys = self.system.lock().expect("system lock");
                        sys.replace_nodes(nodes);
                    }
                    UnitEV::ListenToInput => {
                        let mut enabled = self
                            .input_analyzer_enabled
                            .lock()
                            .expect("lock input_analyzer_enabled");
                        *enabled = true;
                    }
                    UnitEV::IgnoreInput => {
                        let mut enabled = self
                            .input_analyzer_enabled
                            .lock()
                            .expect("lock input_analyzer_enabled");
                        *enabled = false;
                        let nodes = self.nodes.lock().expect("lock nodes");
                        for (_, node) in nodes.iter() {
                            node.control.set_value(0.0)
                        }
                    }
                    UnitEV::Suspend => {
                        cfg_if::cfg_if! {
                            if #[cfg(not(feature = "browser"))] {
                                IN_STREAM.with(|mtx| {
                                    if let Ok(mtx) = mtx.lock() {
                                        if let Some(stream) = mtx.as_ref() {
                                            cpal::traits::StreamTrait::pause(stream).expect("pause input");
                                        }
                                    }
                                });
                                OUT_STREAM.with(|mtx| {
                                    if let Ok(mtx) = mtx.lock() {
                                        if let Some(stream) = mtx.as_ref() {
                                            cpal::traits::StreamTrait::pause(stream).expect("pause output");
                                        }
                                    }
                                });
                            }
                        }
                    }
                    UnitEV::Resume => {
                        cfg_if::cfg_if! {
                            if #[cfg(not(feature = "browser"))] {
                                IN_STREAM.with(|mtx| {
                                    if let Ok(mtx) = mtx.lock() {
                                        if let Some(stream) = mtx.as_ref() {
                                            cpal::traits::StreamTrait::play(stream).expect("play input");
                                        }
                                    }
                                });
                                OUT_STREAM.with(|mtx| {
                                    if let Ok(mtx) = mtx.lock() {
                                        if let Some(stream) = mtx.as_ref() {
                                            cpal::traits::StreamTrait::play(stream).expect("play output");
                                        }
                                    }
                                });
                            }
                        }
                    }
                }
            }
        }

        #[cfg(not(feature = "worklet"))]
        self.resolve_sender
            .unbounded_send(UnitResolve::UpdateEV(true))
            .unwrap();
    }

    pub fn backends(&self) -> (NetBackend32, NetBackend32) {
        let mut sys = self.system.lock().expect("system lock");
        (sys.input_net.backend(), sys.output_net.backend())
    }

    pub fn run(
        &mut self,
        #[cfg(not(feature = "worklet"))] fft_prod: StaticProducer<
            'static,
            Vec<(f32, f32)>,
            FFT_BUF_SIZE,
        >,
        #[cfg(not(feature = "worklet"))] snoops_prod: StaticProducer<
            'static,
            Vec<(Entity, Vec<f32>)>,
            SNOOPS_BUF_SIZE,
        >,
        #[cfg(feature = "worklet")] callback: &Mutex<Box<dyn FnMut(&[f32]) -> [[f32; 128]; 2]>>,
    ) -> Result<()> {
        let (input_be, render_be) = {
            let (input, output) = self.backends();
            (
                BigBlockAdapter32::new(Box::new(input)),
                BlockRateAdapter32::new(Box::new(output)),
            )
        };

        cfg_if::cfg_if! { if #[cfg(feature = "browser")] {
            cfg_if::cfg_if! { if #[cfg(feature = "worklet")] {
                let mut callback = callback.lock().unwrap();
                *callback = self.make_callback(input_be, render_be);
            } else {
                let unit = self.clone();
                let resolve = self.resolve_sender.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match unit.run_audio_ctx(fft_prod, snoops_prod).await {
                        Ok(_) => {
                            resolve.unbounded_send(UnitResolve::RunUnit(true)).unwrap();
                        }
                        Err(e) => {
                            log::error!("run unit error: {e:?}");
                            resolve.unbounded_send(UnitResolve::RunUnit(false)).unwrap();
                        }
                    }
                });
            }}
        } else {
            log::info!("run cpal");
            match self.run_cpal_streams(input_be, render_be, fft_prod, snoops_prod) {
                Ok(_) => self.resolve_sender.unbounded_send(UnitResolve::RunUnit(true))?,
                Err(e) = > {
                    log::error!("run unit error: {e:?}");
                    self.resolve_sender.unbounded_send(UnitResolve::RunUnit(false))?;
                }
            }
        }};

        Ok(())
    }

    #[cfg(all(feature = "browser", not(feature = "worklet")))]
    async fn run_audio_ctx(
        &self,
        mut fft_prod: StaticProducer<'static, Vec<(f32, f32)>, FFT_BUF_SIZE>,
        mut snoops_prod: StaticProducer<'static, Vec<(Entity, Vec<f32>)>, SNOOPS_BUF_SIZE>,
    ) -> Result<()> {
        use js_sys::{Function, Object, Promise, Reflect, Uint8Array};
        use wasm_bindgen::{closure::Closure, JsCast, JsValue};
        use wasm_bindgen_futures::JsFuture;
        use web_sys::*;

        let ctx = AudioContext::new().map_err(map_js_err)?;
        let window = window().unwrap();

        let node = match AudioWorkletNode::new(&ctx, "red-siren-processor") {
            Ok(node) => node,
            Err(_) => {
                let worklet = ctx.audio_worklet().map_err(map_js_err)?;
                let loading = worklet
                    .add_module("/worklet/au_core.js")
                    .map_err(map_js_err)?;
                log::info!("load module");

                JsFuture::from(loading).await.map_err(map_js_err)?;

                AudioWorkletNode::new(&ctx, "red-siren-processor").map_err(map_js_err)?
            }
        };

        let response = JsFuture::from(window.fetch_with_str("/worklet/wasm/au_core_bg.wasm"))
            .await
            .map_err(map_js_err)?;
        let response = Response::from(response);
        let promise = response.array_buffer().map_err(map_js_err)?;
        let wasm_bytes = JsFuture::from(promise).await.map_err(map_js_err)?;
        let port = node.port().map_err(map_js_err)?;

        let mut send_bytes = |resolve: Function, _: Function| {
            let listener = Closure::wrap(Box::new(move |ev: JsValue| {
                let ev = MessageEvent::from(ev);
                let data = ev.data();
                let ev_type = Reflect::get(&data, &"type".into())
                    .expect("ev type")
                    .as_string()
                    .expect("ev type");

                if ev_type.as_str() == "wasm_ready" {
                    resolve.call0(&JsValue::NULL);
                }
            }) as Box<dyn FnMut(JsValue)>);

            port.set_onmessage(Some(listener.as_ref().unchecked_ref()));

            let message = Object::new();
            Reflect::set(&message, &"type".into(), &"wasm".into()).unwrap();
            Reflect::set(&message, &"value".into(), &wasm_bytes).unwrap();

            port.post_message(&message).unwrap();

            std::mem::forget(listener);
        };

        let ready_promise = Promise::new(&mut send_bytes);
        JsFuture::from(ready_promise).await.map_err(map_js_err)?;

        let port = node.port().map_err(map_js_err)?;
        let on_message = Closure::wrap(Box::new(move |ev: JsValue| {
            let ev = MessageEvent::from(ev);
            let data = ev.data();
            let ev_type = Reflect::get(&data, &"type".into())
                .expect("ev type")
                .as_string()
                .expect("ev type");
            let value = Reflect::get(&data, &"value".into())
                .ok()
                .map(|v| Uint8Array::from(v));

            log::trace!("unit received ev: {}", ev_type.as_str());

            match (ev_type.as_str(), value) {
                ("snoops_data", Some(arr)) => {
                    let snoops_data =
                        bincode::deserialize::<Vec<(Entity, Vec<f32>)>>(&arr.to_vec())
                            .expect("deserialize");

                    if let Err(_) = snoops_prod.push(snoops_data) {
                        log::warn!("snoops sender full");
                    } else {
                        log::trace!("send snoops");
                    }
                }
                ("fft_data", Some(arr)) => {
                    let fft_data = bincode::deserialize::<Vec<(f32, f32)>>(&arr.to_vec())
                        .expect("deserialize");

                    if let Err(_) = fft_prod.push(fft_data) {
                        log::warn!("fft sender full");
                    } else {
                        log::trace!("send fft");
                    }
                }
                _ => {
                    unimplemented!("event type: {ev_type}")
                }
            }
        }) as Box<dyn FnMut(JsValue)>);

        port.set_onmessage(Some(on_message.as_ref().unchecked_ref()));

        let dst = ctx.destination();
        node.connect_with_audio_node(&dst).map_err(map_js_err)?;

        let navigator = window.navigator();
        let md = navigator.media_devices().map_err(map_js_err)?;
        let mut constraints = MediaStreamConstraints::new();
        constraints.audio(&true.into());
        let query_device = md
            .get_user_media_with_constraints(&constraints)
            .map_err(map_js_err)?;

        let stream = JsFuture::from(query_device).await.map_err(map_js_err)?;
        let stream = MediaStream::from(stream);
        let src_options = MediaStreamAudioSourceOptions::new(&stream);
        let src = MediaStreamAudioSourceNode::new(&ctx, &src_options).map_err(map_js_err)?;

        src.connect_with_audio_node(&node).map_err(map_js_err)?;

        ON_MESSAGE.with(move |mtx| {
            let mut mtx = mtx.lock().unwrap();
            *mtx = on_message;
        });

        NODE.with(move |mtx| {
            let mut mtx = mtx.lock().unwrap();
            _ = mtx.insert(node);
        });

        CTX.with(move |mtx| {
            let mut mtx = mtx.lock().unwrap();
            _ = mtx.insert(ctx);
        });

        log::info!("running audio");

        Ok(())
    }

    #[cfg(feature = "worklet")]
    fn make_callback(
        &mut self,
        mut input_be: BigBlockAdapter32,
        mut render_be: BlockRateAdapter32,
    ) -> Box<dyn FnMut(&[f32]) -> [[f32; 128]; 2]> {
        let unit = self.clone();

        let rn_rb = HeapRb::<(f32, f32)>::new(self.buffer_size as usize * RENDER_BURSTS);
        let an_rb = HeapRb::<f32>::new(self.fft_res as usize * RENDER_BURSTS);
        let (mut produce_analyze, mut consume_analyze) = an_rb.split();
        let (mut produce_render, mut consume_render) = rn_rb.split();
        let (fft_prod, fft_cons) = channel();
        let (snoops_prod, snoop_cons) = channel();

        _ = self
            .consumers
            .lock()
            .unwrap()
            .insert((fft_cons, snoop_cons));

        let mut exchange_buffer = [[0_f32; 128]; 2];
        Box::new(move |data: &[f32]| {
            unit.store_input(data, &mut produce_analyze, &mut input_be)
                .unwrap();
            unit.process_input(&mut consume_analyze, &fft_prod).unwrap();

            unit.produce_output(&mut produce_render, &mut render_be)
                .unwrap();
            unit.send_snoops_reading(&snoops_prod).unwrap();

            let (ch1, ch2): (Vec<f32>, Vec<f32>) = consume_render
                .pop_iter()
                .take(unit.buffer_size as usize)
                .unzip();

            exchange_buffer[0].clone_from_slice(ch1.as_slice());
            exchange_buffer[1].clone_from_slice(ch2.as_slice());

            exchange_buffer.clone()
        })
    }

    fn process_input(
        &self,
        consume_analyze: &mut HeapConsumer<f32>,
        fft_prod: &mut StaticProducer<Vec<(f32, f32)>, FFT_BUF_SIZE>,
    ) -> Result<()> {
        if consume_analyze.len() >= self.fft_res {
            if let Ok(nodes) = self.nodes.lock() {
                let samples = consume_analyze
                    .pop_iter()
                    .take(self.fft_res)
                    .collect::<Vec<_>>();
                let fft_data = process_input_data(samples.as_slice(), &nodes, self.sample_rate);
                match fft_prod.push(fft_data) {
                    Ok(_) => {}
                    Err(_) => {
                        log::warn!("fft prod full")
                    }
                }
            }
        }

        Ok(())
    }

    fn store_input(
        &self,
        data: &[f32],
        produce_analyze: &mut HeapProducer<f32>,
        input_be: &mut BigBlockAdapter32,
    ) -> Result<()> {
        let mut output_frames = vec![0.0_f32; data.len()];
        let is_enabled = self
            .input_analyzer_enabled
            .lock()
            .expect("lock process_input_analyzer_enabled");

        if *is_enabled {
            input_be.process(data.len(), &[data], &mut [output_frames.as_mut_slice()]);
            let mut it = output_frames.into_iter();
            produce_analyze.push_iter(&mut it);
        }

        Ok(())
    }

    fn produce_output(
        &self,
        produce: &mut HeapProducer<(f32, f32)>,
        render_be: &mut BlockRateAdapter32,
    ) -> Result<()> {
        let overflow = produce.is_full();

        if !overflow {
            for _ in 0..Ord::min(self.buffer_size as usize, produce.free_len()) {
                produce.push(render_be.get_stereo()).unwrap();
            }
        }

        Ok(())
    }

    #[cfg(feature = "worklet")]
    pub fn next_fft_reading(&self) -> Option<Vec<(f32, f32)>> {
        let mut recv = self.consumers.lock().unwrap();
        let (recv, _) = recv.as_mut().unwrap();
        recv.try_recv().ok()
    }

    #[cfg(feature = "worklet")]
    pub fn next_snoops_reading(&self) -> Option<Vec<(Entity, Vec<f32>)>> {
        let mut recv = self.consumers.lock().unwrap();
        let (_, recv) = recv.as_mut().unwrap();
        recv.try_recv().ok()
    }

    fn send_snoops_reading(
        &self,
        snoops_prod: &mut StaticProducer<Vec<(Entity, Vec<f32>)>, SNOOPS_BUF_SIZE>,
    ) -> Result<()> {
        if let Ok(system) = self.system.try_lock() {
            if let Ok(mut snoops) = system.snoops.try_lock() {
                let snoops = snoops
                    .iter_mut()
                    .map(|(s, e)| (e, s.get()))
                    .collect::<Vec<_>>();

                if snoops.iter().any(|(_, s)| s.is_some()) {
                    let data = snoops
                        .into_iter()
                        .map(|(e, snoop)| {
                            let mut data = vec![];
                            if let Some(buf) = snoop {
                                for i in 0..Ord::min(crate::SNOOP_SIZE, buf.size()) {
                                    data.push(buf.at(i))
                                }
                            }
                            (*e, data)
                        })
                        .collect();

                    match snoops_prod.push(data) {
                        Ok(_) => {}
                        Err(_) => {
                            log::warn!("snoops prod full")
                        }
                    }
                }
            }
        }

        Ok(())
    }

    #[cfg(not(feature = "browser"))]
    fn run_cpal_streams(
        &mut self,
        mut input_be: BigBlockAdapter32,
        mut render_be: BlockRateAdapter32,
        fft_prod: StaticProducer<Vec<(f32, f32)>>,
        snoops_prod: StaticProducer<Vec<(Entity, Vec<f32>)>>,
    ) -> Result<()> {
        let host = cpal::default_host();
        let input = cpal::traits::HostTrait::default_input_device(&host);
        let output = cpal::traits::HostTrait::default_output_device(&host);

        log::info!(
            "using input: {:?}",
            input
                .as_ref()
                .map(|d| cpal::traits::DeviceTrait::name(d).ok())
                .flatten()
        );
        log::info!(
            "using output: {:?}",
            output
                .as_ref()
                .map(|d| cpal::traits::DeviceTrait::name(d).ok())
                .flatten()
        );

        let err_fn = |e: cpal::StreamError| {
            log::error!("stream error: {e}");
        };

        log::debug!("buffer size: {}", self.buffer_size);
        let rn_rb = HeapRb::<(f32, f32)>::new(self.buffer_size as usize * RENDER_BURSTS);
        let an_rb = HeapRb::<f32>::new(self.fft_res as usize * RENDER_BURSTS);
        let (mut produce_analyze, mut consume_analyze) = an_rb.split();
        let (mut produce_render, mut consume_render) = rn_rb.split();

        let unit = self.clone();
        let mut process_fn = move || -> Result<()> {
            unit.process_input(&mut consume_analyze, &fft_prod)?;
            unit.produce_output(&mut produce_render, &mut render_be)?;
            unit.send_snoops_reading(&snoops_prod)?;
            Ok(())
        };

        POOL.with(move |pool| {
            pool.spawn_ok(async move {
                loop {
                    if let Err(e) = process_fn() {
                        log::error!("processing error: {e:?}");
                        break;
                    }
                }
            });
        });

        let unit = self.clone();
        let in_stream = input
            .map(move |input| {
                if let Ok(config) = cpal::traits::DeviceTrait::default_input_config(&input) {
                    let config: cpal::StreamConfig = config.into();
                    let input_data_fn = move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        unit.store_input(data, &mut produce_analyze, &mut input_be)
                            .unwrap();
                    };
                    let stream = cpal::traits::DeviceTrait::build_input_stream(
                        &input,
                        &config,
                        input_data_fn,
                        err_fn,
                        None,
                    )
                    .expect("create stream");
                    cpal::traits::StreamTrait::play(&stream)
                        .ok()
                        .map(|_| stream)
                } else {
                    None
                }
            })
            .flatten();

        let out_stream = output
            .map(move |output| {
                if let Ok(config) = cpal::traits::DeviceTrait::default_output_config(&output) {
                    let config: cpal::StreamConfig = config.into();

                    let output_data_fn = move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                        let mut underflow = 0;
                        for frame in data.chunks_mut(config.channels as usize) {
                            if let Some(sample) = consume_render.pop() {
                                if config.channels == 1 {
                                    frame[0] = (sample.0 * sample.1) / 2.0
                                } else {
                                    frame[0] = sample.0;
                                    frame[1] = sample.1;
                                }
                            } else {
                                underflow += 1;
                            }
                        }

                        if underflow > 0 {
                            log::warn!("underflow: {underflow}")
                        }
                    };

                    let stream = cpal::traits::DeviceTrait::build_output_stream(
                        &output,
                        &config,
                        output_data_fn,
                        err_fn,
                        None,
                    )
                    .expect("create stream");
                    cpal::traits::StreamTrait::play(&stream)
                        .ok()
                        .map(|_| stream)
                } else {
                    None
                }
            })
            .flatten();

        if let Some(in_stream) = in_stream {
            IN_STREAM.with(move |mtx| {
                let mut stream = mtx.lock().expect("lock in stream");
                _ = stream.insert(in_stream);
            });
        }
        if let Some(out_stream) = out_stream {
            OUT_STREAM.with(move |mtx| {
                let mut stream = mtx.lock().expect("lock out stream");
                _ = stream.insert(out_stream);
            });
            Ok(())
        } else {
            Err(anyhow!("Could not create output"))
        }
    }
}

#[cfg(feature = "browser")]
#[allow(dead_code)]
fn map_js_err(err: wasm_bindgen::JsValue) -> Error {
    let description = format!("{:?}", err);
    anyhow!("js err: {description}")
}

pub fn process_input_data(
    samples: &[f32],
    nodes: &HashMap<Entity, Node>,
    sample_rate: u32,
) -> Vec<(f32, f32)> {
    let hann_window = hann_window(samples);

    let spectrum_hann_window = samples_fft_to_spectrum(
        &hann_window,
        sample_rate,
        FrequencyLimit::Range(MIN_F, MAX_F),
        Some(&divide_by_N_sqrt),
    )
    .unwrap();

    let data = spectrum_hann_window
        .data()
        .iter()
        .map(|(f, v)| (f.val(), v.val()))
        .collect::<Vec<_>>();

    for (_, node) in nodes.iter() {
        let (min_fq, max_fq) = (node.f_sense.0 .0.value(), node.f_sense.0 .1.value());
        let (min_value, max_value) = (node.f_sense.1 .0.value(), node.f_sense.1 .1.value());
        let n_breadth = data
            .iter()
            .filter(|(freq, _)| *freq >= min_fq && *freq <= max_fq)
            .count();

        let activation = data.iter().fold(0.0, |acc, (freq, value)| {
            if *freq >= min_fq && *freq <= max_fq && *value >= min_value && *value <= max_value {
                acc + (1.0 / n_breadth as f32)
            } else {
                acc
            }
        });

        if activation > 0.0 {
            log::info!("activated node {} by {}", node.f_base.value(), activation)
        } else if node.control.value() > 0.0 {
            log::info!("deactivated node {}", node.f_base.value())
        }

        node.control.set_value(activation)
    }

    data
}
