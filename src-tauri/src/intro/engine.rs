use parking_lot::Mutex;
use std::{
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::Duration,
};

use fundsp::hacker32::*;

pub(super) enum Control {
    Pause,
    Resume,
    Shutdown,
}

pub struct IntroEngineState {
    pub(super) inner: Mutex<Inner>,
}

pub(super) struct Inner {
    pub started: bool,
    pub paused: bool,
    pub tx: Option<Sender<Control>>,
    pub join: Option<thread::JoinHandle<()>>,
    // Front snoops (read side) shared with request command.
    pub snoops: Vec<Snoop>,
}

impl IntroEngineState {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Inner {
                started: false,
                paused: false,
                snoops: Vec::new(),
                tx: None,
                join: None,
            }),
        }
    }
}

impl Drop for IntroEngineState {
    fn drop(&mut self) {
        let mut inner = self.inner.lock();
        if let Some(tx) = inner.tx.take() {
            let _ = tx.send(Control::Shutdown);
        }
        if let Some(handle) = inner.join.take() {
            let _ = handle.join();
        }
    }
}

pub(super) struct EngineConfig {
    num_snoops: usize,
    sample_rate: f32,
    base_freq: f32,
    mod_freq: f32,
    max_depth: f32,
    amplitude: f32,
    buffer_size: usize,
}

// Slow base, preserved 0.53 cycles shape via longer duration
const INTRO_NUM_SNOOPS: usize = 11;
const INTRO_BASE_FREQ_HZ: f32 = 0.085;
const INTRO_SAMPLE_RATE_HZ: f32 = 120.0;
const INTRO_MOD_FREQ_HZ: f32 = 0.005;
const INTRO_MAX_DEPTH: f32 = 0.5;
const INTRO_AMPLITUDE: f32 = 0.85;

// Duration ≈ 7.2 s -> matches shape of 0.33 Hz @ 1.6 s
const INTRO_BUFFER_SIZE: usize = 864;

// Engine pacing (tweak if jitter): ~2 ms sleep
const INTRO_THREAD_SLEEP_US: u64 = 2000;

// Decimation stride used when harvesting samples:
pub(super) const INTRO_DECIMATION_STRIDE: usize = 6;

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            num_snoops: INTRO_NUM_SNOOPS,
            sample_rate: INTRO_SAMPLE_RATE_HZ,
            base_freq: INTRO_BASE_FREQ_HZ,
            mod_freq: INTRO_MOD_FREQ_HZ,
            max_depth: INTRO_MAX_DEPTH,
            amplitude: INTRO_AMPLITUDE,
            buffer_size: INTRO_BUFFER_SIZE,
        }
    }
}

struct FundspEngine {
    // Mono collapsed graph (after join); internal structure still contains 11 snoop taps.
    net: Net,
    depths: Vec<f32>,
}

impl FundspEngine {
    fn new(cfg: &EngineConfig) -> (Self, Vec<Snoop>) {
        assert_eq!(cfg.num_snoops, INTRO_NUM_SNOOPS);

        // Pre-create snoop front/back pairs so we can avoid Rc/RefCell capture.
        let mut fronts: Vec<Snoop> = Vec::with_capacity(cfg.num_snoops);
        let mut backs: Vec<An<SnoopBackend>> = Vec::with_capacity(cfg.num_snoops);
        for _ in 0..INTRO_NUM_SNOOPS {
            let (front, back) = snoop(cfg.buffer_size);
            fronts.push(front);
            backs.push(back);
        }

        // Precompute depths (linear ramp)
        let depths: Vec<f32> = (0..cfg.num_snoops)
            .map(|i| {
                (if cfg.num_snoops <= 1 {
                    0.0
                } else {
                    cfg.max_depth * (i as f32) / (cfg.num_snoops as f32 - 1.0)
                }) + cfg.max_depth
            })
            .collect();

        log::debug!("nodes depth: {depths:?}");

        // Copy data for closure capture.
        let mod_freq = cfg.mod_freq;
        let base_freq = cfg.base_freq;
        let amp = cfg.amplitude;
        let depths_for_closure = depths.clone();

        let gen = pink() | (dc(1.0) + saw_hz(mod_freq)) | sine_hz(base_freq); // | pink();

        // Build parallel bus (11 branches).
        let bus = busi::<U11, _, _>(move |k| {
            let idx = k as usize;
            let depth = depths_for_closure[idx];
            let snoop_be = backs[idx].clone();
            // Apply branch-specific depth and amplitude modulation
            //(((pass() * depth) * (pass() * amp)) * pass())

            ((pass() + ((pass() * depth) * pass())) * (amp * depth)) >> declick() >> snoop_be
        });

        // Collapse multi-channel bus to one mono output (not used, just drives ticking).
        let mut net = Net::new(0, 1);

        net.set_sample_rate(cfg.sample_rate as f64);

        let complete_network = gen >> bus;
        let node = net.push(Box::new(complete_network));

        net.connect_output(node, 0, 0);

        (Self { net, depths }, fronts)
    }

    fn tick_frame(&mut self) {
        let _ = self.net.get_mono();
    }
}

// -----------------------------------------------------------------------------
// Thread & Run Loop
// -----------------------------------------------------------------------------
pub(super) fn spawn_engine(
    paused: bool,
    config: EngineConfig,
) -> (
    Sender<Control>,
    thread::JoinHandle<()>,
    Vec<Snoop>,
    Vec<f32>,
) {
    // Build engine on this thread so we can extract snoops (fronts) & depths.
    let (engine, snoops) = FundspEngine::new(&config);
    let depths = engine.depths.clone();
    let (tx, rx) = mpsc::channel();
    let handle = thread::spawn(move || run_engine(config, rx, paused, engine));
    (tx, handle, snoops, depths)
}

fn run_engine(
    _config: EngineConfig,
    rx: Receiver<Control>,
    mut paused: bool,
    mut engine: FundspEngine,
) {
    let rx = rx;
    loop {
        while let Ok(msg) = rx.try_recv() {
            match msg {
                Control::Pause => paused = true,
                Control::Resume => paused = false,
                Control::Shutdown => return,
            }
        }
        if paused {
            thread::sleep(Duration::from_millis(20));
            continue;
        }
        for _ in 0..3 {
            engine.tick_frame();
        }
        if INTRO_THREAD_SLEEP_US > 0 {
            std::thread::sleep(std::time::Duration::from_micros(INTRO_THREAD_SLEEP_US));
        }
    }
}
