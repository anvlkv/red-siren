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
```json
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
```
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
    time::Duration,
};

use tauri::{App, Manager, State};

use ::shared::events::intro::{IntroSnoopBatchPayload, IntroSnoopSample};
use fundsp::hacker32::*; // brings in busi, join, U-types, oscillators, dc, etc.

pub fn setup(app: &mut App) -> Result<(), String> {
    app.manage(IntroEngineState::new());

    Ok(())
}

// -----------------------------------------------------------------------------
// Control messages & shared state
// -----------------------------------------------------------------------------
enum Control {
    Pause,
    Resume,
    Shutdown,
}

pub struct IntroEngineState {
    inner: Mutex<Inner>,
}

struct Inner {
    started: bool,
    paused: bool,
    tx: Option<Sender<Control>>,
    join: Option<thread::JoinHandle<()>>,
    // Front snoops (read side) shared with request command.
    snoops: Vec<Snoop>,
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
    buffer_size: usize,
}

const INTRO_NUM_SNOOPS: usize = 11;
const INTRO_SAMPLE_RATE_HZ: f32 = 96.0;    // rebalanced
const INTRO_BASE_FREQ_HZ: f32 = 0.33;
const INTRO_MOD_FREQ_HZ: f32 = 0.00165;    // scaled (optional)
const INTRO_MAX_DEPTH: f32 = 0.5;
const INTRO_AMPLITUDE: f32 = 0.52;
const INTRO_BUFFER_SIZE: usize = 480;      // ~96 * 1.6 / 0.33
const INTRO_THREAD_SLEEP_US: u64 = 2000;   // coarse pacing; reduce if phase jitter noticeable





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

// -----------------------------------------------------------------------------
// Fundsp Engine
// -----------------------------------------------------------------------------
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
                if cfg.num_snoops <= 1 {
                    0.0
                } else {
                    cfg.max_depth * (i as f32) / (cfg.num_snoops as f32 - 1.0)
                }
            })
            .rev()
            .collect();

        // Copy data for closure capture.
        let mod_freq = cfg.mod_freq;
        let base_freq = cfg.base_freq;
        let amp = cfg.amplitude;
        let depths_for_closure = depths.clone();

        // Build parallel bus (11 branches).
        let bus = busi::<U11, _, _>(move |k| {
            let idx = k as usize;
            let depth = depths_for_closure[idx];
            let amp_line = ((idx + INTRO_NUM_SNOOPS / 3) as f32 / INTRO_NUM_SNOOPS as f32) * amp;
            let snoop_be = backs[idx].clone();
            // (1 + depth * saw) * sine * amplitude >> pre-built snoop backend
            ((dc(1.0) + saw_hz(mod_freq) * depth) * sine_hz(base_freq) * amp_line)
                >> declick()
                >> snoop_be
        });

        // Collapse multi-channel bus to one mono output (not used, just drives ticking).
        let mut net = Net::new(0, 1);

        net.set_sample_rate(cfg.sample_rate as f64);

        let node = net.push(Box::new(bus));

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
fn spawn_engine(
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

// -----------------------------------------------------------------------------
// Tauri Commands
// -----------------------------------------------------------------------------

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

// -----------------------------------------------------------------------------
// New on-demand frame command
// -----------------------------------------------------------------------------
#[tauri::command]
pub async fn intro_next_frame(
    state: State<'_, IntroEngineState>,
) -> Result<IntroSnoopBatchPayload, String> {
    use std::time::{SystemTime, UNIX_EPOCH};

    // Lazy start engine if not running.
    {
        let mut inner = state
            .inner
            .lock()
            .map_err(|_| "intro_engine_state_poisoned".to_string())?;
        if !inner.started {
            let config = EngineConfig::default();
            let (tx, handle, snoops, _depths) = spawn_engine(inner.paused, config);
            inner.tx = Some(tx);
            inner.join = Some(handle);
            inner.snoops = snoops;
            inner.started = true;
        }
    }

    // Collect snapshot.
    let mut inner = state
        .inner
        .lock()
        .map_err(|_| "intro_engine_state_poisoned".to_string())?;
    if inner.snoops.is_empty() {
        return Err("intro_engine_not_ready".into());
    }

    let num = inner.snoops.len();
    let max_depth = INTRO_MAX_DEPTH;
    let t_unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "time_error".to_string())?
        .as_millis() as u64;

    let mut snoop_payloads = Vec::with_capacity(num);
    for (i, snoop) in inner.snoops.iter_mut().enumerate() {
        snoop.update();
        let cap = snoop.capacity();
        let mut samples = Vec::with_capacity(cap);
        // Reverse chronological (latest first) -> make chronological oldest→newest as before.
        for rev in (0..cap){
            samples.push(snoop.at(rev));
        }
        let depth = if num > 1 {
            max_depth * (i as f32) / (num as f32 - 1.0)
        } else {
            0.0
        };
        snoop_payloads.push(IntroSnoopSample {
            snoop_id: (i + 1) as u8,
            modulation_depth: depth,
            samples,
        });
    }

    Ok(IntroSnoopBatchPayload {
        t_unix_ms,
        snoops: snoop_payloads,
    })
}
