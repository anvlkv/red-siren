use std::{
    collections::HashMap,
    sync::{
        mpsc::{SyncSender, TrySendError},
        Arc, Mutex,
    },
};

use crate::{Node, System, MAX_F, MIN_F, SNOOP_SIZE};
use anyhow::Result;
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    Stream, SupportedBufferSize,
};
use fundsp::hacker32::*;
use futures::executor::ThreadPool;
use hecs::Entity;
use ringbuf::HeapRb;
use spectrum_analyzer::{
    samples_fft_to_spectrum, scaling::divide_by_N_sqrt, windows::hann_window, FrequencyLimit,
};

thread_local! {
    static IN_STREAM: Arc<Mutex<Option<Stream>>> = Default::default();
    static OUT_STREAM: Arc<Mutex<Option<Stream>>> = Default::default();
    static POOL: ThreadPool = ThreadPool::new().expect("create pool");
}

const RENDER_BURSTS: usize = 5;

pub struct Unit {
    fft_res: usize,
    sample_rate: u32,
    buffer_size: u32,
    system: Arc<Mutex<System>>,
    snoops: Arc<Mutex<Vec<(Snoop<f32>, Entity)>>>,
    nodes: Arc<Mutex<HashMap<Entity, Node>>>,
    input_analyzer_enabled: Arc<Mutex<bool>>,
}

impl Default for Unit {
    fn default() -> Self {
        Self::new()
    }
}

pub enum UnitEV {
    ButtonPressed(Entity),
    ButtonReleased(Entity),
    Detune(Entity, f32),
    Configure(Vec<Node>),
    SetControl(Entity, f32),
    Suspend,
    Resume,
    ListenToInput,
    IgnoreInput,
}

impl Unit {
    pub fn new() -> Self {
        let (sample_rate, buffer_size) = cpal::default_host()
            .default_output_device()
            .map(|d| d.default_output_config().ok())
            .flatten()
            .map(|d| {
                (
                    d.sample_rate().0,
                    match d.buffer_size() {
                        SupportedBufferSize::Range { min: _, max } => *max,
                        SupportedBufferSize::Unknown => 1024,
                    },
                )
            })
            .unwrap_or((44100, 1024));

        let fft_res = 1024;

        let sys = System::new(sample_rate);
        let nodes = sys.nodes.clone();
        let snoops = sys.snoops.clone();
        let system = Arc::new(Mutex::new(sys));

        Self {
            fft_res,
            system,
            nodes,
            snoops,
            sample_rate,
            buffer_size,
            input_analyzer_enabled: Arc::new(Mutex::new(true)),
        }
    }

    pub fn update(&mut self, ev: UnitEV) {
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
                IN_STREAM.with(|mtx| {
                    if let Ok(mtx) = mtx.lock() {
                        if let Some(stream) = mtx.as_ref() {
                            stream.pause().expect("pause input")
                        }
                    }
                });
                OUT_STREAM.with(|mtx| {
                    if let Ok(mtx) = mtx.lock() {
                        if let Some(stream) = mtx.as_ref() {
                            stream.pause().expect("pause output")
                        }
                    }
                });
            }
            UnitEV::Resume => {
                IN_STREAM.with(|mtx| {
                    if let Ok(mtx) = mtx.lock() {
                        if let Some(stream) = mtx.as_ref() {
                            stream.play().expect("play input")
                        }
                    }
                });
                OUT_STREAM.with(|mtx| {
                    if let Ok(mtx) = mtx.lock() {
                        if let Some(stream) = mtx.as_ref() {
                            stream.play().expect("play output")
                        }
                    }
                });
            }
        }
    }

    pub fn run(
        &mut self,
        fft_sender: SyncSender<Vec<(f32, f32)>>,
        snoops_sender: SyncSender<Vec<(Entity, Vec<f32>)>>,
    ) -> Result<()> {
        let (mut render_be, mut input_be) = {
            let mut sys = self.system.lock().expect("system lock");
            (
                BlockRateAdapter32::new(Box::new(sys.output_net.backend())),
                BigBlockAdapter32::new(Box::new(sys.input_net.backend())),
            )
        };

        let host = cpal::default_host();
        let input = host.default_input_device();
        let output = host.default_output_device();

        log::info!(
            "using input: {:?}",
            input.as_ref().map(|d| d.name().ok()).flatten()
        );
        log::info!(
            "using output: {:?}",
            output.as_ref().map(|d| d.name().ok()).flatten()
        );

        let err_fn = |e: cpal::StreamError| {
            log::error!("stream error: {e}");
        };

        let rn_rb = HeapRb::<(f32, f32)>::new(self.buffer_size as usize * RENDER_BURSTS);
        let an_rb = HeapRb::<f32>::new(self.fft_res as usize * RENDER_BURSTS);
        let (mut produce_analyze, mut consume_analyze) = an_rb.split();
        let (mut produce, mut consume) = rn_rb.split();

        let process_nodes = self.nodes.clone();
        let process_snoops = self.snoops.clone();
        let fft_res = self.fft_res;
        let buffer_size = self.buffer_size;
        let sample_rate = self.sample_rate;
        POOL.with(move |pool| {
            pool.spawn_ok(async move {
                loop {
                    let _tmr = logging_timer::timer!("TICK");
                    let overflow = produce.is_full();
    
                    if !overflow {
                        for _ in 0..Ord::min(buffer_size as usize, produce.free_len()) {
                            produce.push(render_be.get_stereo()).expect("send render");
                        }
                    }
    
                    if consume_analyze.len() >= fft_res {
                        if let Ok(nodes) = process_nodes.lock() {
                            let samples = consume_analyze.pop_iter().take(fft_res).collect::<Vec<_>>();
                            let fft_data = process_input_data(samples.as_slice(), &nodes, sample_rate);
                            match fft_sender.try_send(fft_data) {
                                Ok(_) => {}
                                Err(TrySendError::Full(_)) => {
                                    log::warn!("skipping fft data")
                                }
                                Err(_) => {
                                    log::error!("fft receiver gone");
                                    break;
                                }
                            }
                        }
                    }
    
                    if let Ok(mut snoops) = process_snoops.try_lock() {
                        let snps = snoops
                            .iter_mut()
                            .map(|(s, e)| (e, s.get()))
                            .collect::<Vec<_>>();
    
                        if snps.iter().any(|(_, s)| s.is_some()) {
                            let data = snps
                                .into_iter()
                                .map(|(e, snp)| {
                                    let mut data = vec![];
                                    if let Some(buf) = snp {
                                        for i in 0..Ord::min(SNOOP_SIZE, buf.size()) {
                                            data.push(buf.at(i))
                                        }
                                    }
                                    (*e, data)
                                })
                                .collect();
    
                            match snoops_sender.try_send(data) {
                                Ok(_) => {}
                                Err(TrySendError::Full(_)) => {
                                    log::warn!("skipping snoops data");
                                }
                                Err(_) => {
                                    log::error!("snoops receiver gone");
                                    break;
                                }
                            }
                        }
                    }
                }
            });
        });

        let process_input_analyzer_enabled = self.input_analyzer_enabled.clone();
        let in_stream = input
            .map(move |input| {
                if let Ok(config) = input.default_input_config() {
                    let config: cpal::StreamConfig = config.into();
                    let input_data_fn = move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        let mut output_frame = vec![0.0_f32];
                        let is_enabled = process_input_analyzer_enabled
                            .lock()
                            .expect("lock process_input_analyzer_enabled");
                        if *is_enabled {
                            for frame in data.chunks(config.channels as usize) {
                                input_be.tick(frame, output_frame.as_mut_slice());
                                match produce_analyze.push(output_frame[0]) {
                                    Ok(_) => {}
                                    Err(_) => {
                                        // log::warn!("analyze buffer full");
                                        break;
                                    }
                                };
                            }
                        }
                    };
                    let stream = input
                        .build_input_stream(&config, input_data_fn, err_fn, None)
                        .expect("create stream");
                    stream.play().ok().map(|_| stream)
                } else {
                    None
                }
            })
            .flatten();

        let out_stream = output
            .map(move |output| {
                if let Ok(config) = output.default_output_config() {
                    let config: cpal::StreamConfig = config.into();

                    let output_data_fn = move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                        let mut underflow = 0;
                        for frame in data.chunks_mut(config.channels as usize) {
                            if let Some(sample) = consume.pop() {
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

                    let stream = output
                        .build_output_stream(&config, output_data_fn, err_fn, None)
                        .expect("create stream");
                    stream.play().ok().map(|_| stream)
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
            Err(anyhow::anyhow!("Could not create output"))
        }
    }
}

fn process_input_data(
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
