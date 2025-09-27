/*!
Intro DSP engine (fundsp) – busi + join based per-frame snoop engine.

MAYA DRY KISS
-------------
- 11 parallel low-frequency generator branches.
- Each branch: (1 + depth(i) * saw(MOD_FREQ)) * sine(BASE_FREQ) * AMPLITUDE >> snoop(buffer_capacity).
- Built via `busi::<U11,...>` for parallel generation, then collapsed to mono with `join::<U11>()`.
- We only need the graph to advance so snoop ring buffers fill; mono output is discarded.
- Depth linearly ramps 0.0 → INTRO_MAX_DEPTH across index.
- Emits JSON envelope identical to prior contract:
  {
    "event": INTRO_SNOOP_BATCH",
    "data": {
      "tUnixMs": u64,
      "snoops": [
         { "snoopId": 1, "modulationDepth": f32, "samples": [...] },
         ...
      ]
    }
  }
- Pause/Resume: stop/resume advancing without losing phase.
- Idempotent stream start: re-invoking swaps the channel.

No audio playback; this is a visual signal source only.
*/

use std::{
    sync::{
        mpsc::{self, Receiver, Sender},
        Mutex,
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde_json::json;
use tauri::ipc::Channel as TauriChannel;
use tauri::State;

use fundsp::hacker32::*; // brings in busi, join, U-types, oscillators, dc, etc.
use ::shared::events::intro::{IntroSnoopBatchPayload, IntroSnoopSample, INTRO_SNOOP_BATCH};

// -----------------------------------------------------------------------------
// Control messages & shared state
// -----------------------------------------------------------------------------
enum Control {
    Pause,
    Resume,
    UpdateChannel(TauriChannel<serde_json::Value>),
    Shutdown,
}

pub struct IntroEngineState {
    inner: Mutex<Inner>,
}

struct Inner {
    started: bool,
    paused: bool,
    channel: Option<TauriChannel<serde_json::Value>>,
    tx: Option<Sender<Control>>,
    join: Option<thread::JoinHandle<()>>,
}

impl IntroEngineState {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Inner {
                started: false,
                paused: false,
                channel: None,
                tx: None,
                join: None,
            }),
        }
    }
}

// -----------------------------------------------------------------------------
// Configuration
// -----------------------------------------------------------------------------
struct EngineConfig {
    num_snoops: usize,
    sample_rate: f32,
    base_freq: f32,
    mod_freq: f32,
    max_depth: f32,
    amplitude: f32,
    buffer_sizes: Vec<usize>,
    frame_interval: Duration,
}

const INTRO_NUM_SNOOPS: usize = 11;
const INTRO_SAMPLE_RATE_HZ: f32 = 75.0;
const INTRO_BASE_FREQ_HZ: f32 = 0.75;
const INTRO_MOD_FREQ_HZ: f32 = 0.25;
const INTRO_MAX_DEPTH: f32 = 0.7;
const INTRO_AMPLITUDE: f32 = 0.85;
const INTRO_FRAME_INTERVAL_MS: u64 = 55;
// Distinct ring capacities (short → long) for visual width variance.
const INTRO_BUFFER_SIZES: [usize; INTRO_NUM_SNOOPS] = [70, 110, 140, 180, 190, 200, 240, 280, 320, 360, 400];

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            num_snoops: INTRO_NUM_SNOOPS,
            sample_rate: INTRO_SAMPLE_RATE_HZ,
            base_freq: INTRO_BASE_FREQ_HZ,
            mod_freq: INTRO_MOD_FREQ_HZ,
            max_depth: INTRO_MAX_DEPTH,
            amplitude: INTRO_AMPLITUDE,
            buffer_sizes: INTRO_BUFFER_SIZES.to_vec(),
            frame_interval: Duration::from_millis(INTRO_FRAME_INTERVAL_MS),
        }
    }
}

// -----------------------------------------------------------------------------
// Fundsp Engine
// -----------------------------------------------------------------------------
struct FundspEngine {
    // Mono collapsed graph (after join); internal structure still contains 11 snoop taps.
    net: Net,
    snoops: Vec<Snoop>,
    depths: Vec<f32>,
    frame_samples: usize,
}

impl FundspEngine {
    fn new(cfg: &EngineConfig) -> Self {
        assert_eq!(cfg.num_snoops, INTRO_NUM_SNOOPS);
        assert_eq!(cfg.buffer_sizes.len(), cfg.num_snoops);

        use std::cell::RefCell;
        use std::rc::Rc;

        // Collect snoop fronts inside the bus closure (Fn, not FnMut).
        let fronts: Rc<RefCell<Vec<Snoop>>> =
            Rc::new(RefCell::new(Vec::with_capacity(cfg.num_snoops)));
        let capture_fronts = fronts.clone();

        // Precompute depths (linear ramp)
        let depths: Vec<f32> = (0..cfg.num_snoops)
            .map(|i| {
                if cfg.num_snoops <= 1 {
                    0.0
                } else {
                    cfg.max_depth * (i as f32) / (cfg.num_snoops as f32 - 1.0)
                }
            })
            .collect();

        // Copy data for closure capture.
        let capacities = cfg.buffer_sizes.clone();
        let mod_freq = cfg.mod_freq;
        let base_freq = cfg.base_freq;
        let amp = cfg.amplitude;
        let depths_for_closure = depths.clone();

        // Build parallel bus (11 branches).
        let bus = busi::<U11, _, _>(move |k| {
            let idx = k as usize;
            let depth = depths_for_closure[idx];
            let capacity = capacities[idx];
            let (front, back) = snoop(capacity);
            capture_fronts.borrow_mut().push(front);
            let amp = ((idx + 1) as f32 / capacities.len() as f32) * amp;
            // (1 + depth * saw) * sine * amplitude >> snoop backend
            ((dc(1.0) + saw_hz(mod_freq) * depth) * sine_hz(base_freq) * amp) >> back
        });

        // Collapse multi-channel bus to one mono output (not used, just drives ticking).
        let mut net = Net::new(0, 1);

        net.set_sample_rate(cfg.sample_rate as f64);

        let node = net.push(Box::new(bus));

        // net.connect_input(0, node, 0);
        net.connect_output(node, 0, 0);

        let frame_samples =
            ((cfg.frame_interval.as_secs_f32()) * cfg.sample_rate).round().max(1.0) as usize;

        let snoops = Rc::try_unwrap(fronts)
            .map_err(|_| "errored")
            .unwrap()
            .into_inner();

        Self {
            net,
            snoops,
            depths,
            frame_samples,
        }
    }

    fn tick_frame(&mut self) {
        for _ in 0..self.frame_samples {
            // Advance one mono sample (joining underlying 11 branches), updating all snoops.
            let _ = self.net.get_mono();
        }
    }

    fn collect(&mut self) -> Vec<Vec<f32>> {
        self.snoops
            .iter_mut()
            .map(|s| {
                s.update();
                let cap = s.capacity();
                let mut v = Vec::with_capacity(cap);
                for rev in (0..cap).rev() {
                    v.push(s.at(rev));
                }
                v
            })
            .collect()
    }
}

// -----------------------------------------------------------------------------
// Thread & Run Loop
// -----------------------------------------------------------------------------
fn spawn_engine(
    mut channel: Option<TauriChannel<serde_json::Value>>,
    paused: bool,
    config: EngineConfig,
) -> (Sender<Control>, thread::JoinHandle<()>) {
    let (tx, rx) = mpsc::channel();
    let handle = thread::spawn(move || run_engine(&config, &rx, &mut channel, paused));
    (tx, handle)
}

fn run_engine(
    config: &EngineConfig,
    rx: &Receiver<Control>,
    channel: &mut Option<TauriChannel<serde_json::Value>>,
    mut paused: bool,
) {
    let mut engine = FundspEngine::new(config);
    let mut last_emit = Instant::now();

    loop {
        // Handle control messages
        while let Ok(msg) = rx.try_recv() {
            match msg {
                Control::Pause => paused = true,
                Control::Resume => paused = false,
                Control::UpdateChannel(ch) => *channel = Some(ch),
                Control::Shutdown => return,
            }
        }

        if paused {
            thread::sleep(Duration::from_millis(20));
            continue;
        }

        engine.tick_frame();

        if last_emit.elapsed() >= config.frame_interval {
            last_emit = Instant::now();

            if let Some(ch) = channel {
                let t_unix_ms = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;

                let buffers = engine.collect();
                let snoops: Vec<IntroSnoopSample> = buffers
                    .into_iter()
                    .enumerate()
                    .map(|(idx, samples)| IntroSnoopSample {
                        snoop_id: (idx + 1) as u8,
                        modulation_depth: engine.depths[idx],
                        samples,
                    })
                    .collect();

                let payload = IntroSnoopBatchPayload { t_unix_ms, snoops };
                let _ = ch.send(json!({
                    "event": INTRO_SNOOP_BATCH,
                    "data": payload
                }));
            }
        }
    }
}

// -----------------------------------------------------------------------------
// Tauri Commands
// -----------------------------------------------------------------------------
#[tauri::command]
pub async fn intro_stream(
    on_event: TauriChannel<serde_json::Value>,
    state: State<'_, IntroEngineState>,
) -> Result<(), String> {
    let mut inner = state
        .inner
        .lock()
        .map_err(|_| "intro_engine_state_poisoned".to_string())?;

    if !inner.started {
        let config = EngineConfig::default();
        let (tx, handle) = spawn_engine(Some(on_event), inner.paused, config);
        inner.tx = Some(tx);
        inner.join = Some(handle);
        inner.started = true;
        log::info!("intro_stream: engine started");
    } else if let Some(tx) = &inner.tx {
        tx.send(Control::UpdateChannel(on_event))
            .map_err(|e| format!("channel_update_failed: {e}"))?;
        log::info!("intro_stream: channel updated");
    }
    Ok(())
}

#[tauri::command]
pub async fn intro_pause(state: State<'_, IntroEngineState>) -> Result<(), String> {
    let mut inner = state
        .inner
        .lock()
        .map_err(|_| "intro_engine_state_poisoned".to_string())?;
    if inner.paused {
        return Ok(());
    }
    if let Some(tx) = &inner.tx {
        tx.send(Control::Pause)
            .map_err(|e| format!("pause_send_failed: {e}"))?;
        inner.paused = true;
        log::info!("intro_pause: paused");
    }
    Ok(())
}

#[tauri::command]
pub async fn intro_resume(state: State<'_, IntroEngineState>) -> Result<(), String> {
    let mut inner = state
        .inner
        .lock()
        .map_err(|_| "intro_engine_state_poisoned".to_string())?;
    if !inner.paused {
        return Ok(());
    }
    if let Some(tx) = &inner.tx {
        tx.send(Control::Resume)
            .map_err(|e| format!("resume_send_failed: {e}"))?;
        inner.paused = false;
        log::info!("intro_resume: resumed");
    }
    Ok(())
}

impl Drop for IntroEngineState {
    fn drop(&mut self) {
        if let Ok(mut inner) = self.inner.lock() {
            if let Some(tx) = inner.tx.take() {
                let _ = tx.send(Control::Shutdown);
            }
            if let Some(handle) = inner.join.take() {
                let _ = handle.join();
            }
        }
    }
}
