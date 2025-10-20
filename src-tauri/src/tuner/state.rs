use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;
use std::time::Duration;

use audio_system::rt::{make_tuner_runtime, TunerRuntime};
use common::error::Result;
use common::tuner::{Config, Layout as TunerLayout, SpectrumData};
use parking_lot::RwLock;
use tauri::{AppHandle, Emitter};

/// Generic, runtime‑agnostic tuner state.
/// All concrete audio / FFT work is delegated to `audio_system::rt::TunerRuntime`.
///
/// MAYA DRY KISS:
/// - Minimal state here (UI + configuration + latest spectrum).
/// - No direct dependency on CPAL or any runtime feature flags.
/// - Polling thread obtains data via trait object; when no runtime feature
///   is enabled, a Null runtime returns `None` and emits nothing.
pub struct TunerState {
    // Persistent config (updated via commands)
    pub tuner_config: RwLock<Config>,

    // Derived / ephemeral layouts
    pub current_layout: RwLock<Option<TunerLayout>>,
    pub last_instrument_layout: RwLock<Option<common::instrument::Layout>>,

    // Latest spectrum snapshot (updated by polling thread)
    pub spectrum_buffer: Arc<RwLock<Option<SpectrumData>>>,

    // Legacy max-hold fields (kept until commands logic is simplified)
    pub max_hold_buffer: RwLock<Vec<f32>>,
    pub max_hold_decay_rate: RwLock<f32>,

    // Mode flags
    pub tuning_mode: RwLock<bool>,

    // Runtime (boxed trait object) + polling control
    runtime: Arc<RwLock<Option<Box<dyn TunerRuntime + Send + Sync>>>>,
    polling_active: Arc<AtomicBool>,
    polling_thread: RwLock<Option<thread::JoinHandle<()>>>,
}

impl Default for TunerState {
    fn default() -> Self {
        Self::new()
    }
}

impl TunerState {
    pub fn new() -> Self {
        Self {
            tuner_config: RwLock::new(Config::default()),
            current_layout: RwLock::new(None),
            last_instrument_layout: RwLock::new(None),
            spectrum_buffer: Arc::new(RwLock::new(None)),
            max_hold_buffer: RwLock::new(Vec::new()),
            max_hold_decay_rate: RwLock::new(0.95),
            tuning_mode: RwLock::new(false),
            runtime: Arc::new(RwLock::new(None)),
            polling_active: Arc::new(AtomicBool::new(false)),
            polling_thread: RwLock::new(None),
        }
    }

    /// Start tuner streaming & spectrum polling.
    /// Idempotent: repeated calls while running are ignored.
    pub fn start_tuner_stream(&self, app: AppHandle) -> Result<()> {
        // Fast path: already active
        if self.polling_active.load(Ordering::SeqCst) {
            return Ok(());
        }

        *self.tuning_mode.write() = true;

        // Ensure runtime exists
        {
            let mut rt_guard = self.runtime.write();
            if rt_guard.is_none() {
                *rt_guard = Some(make_tuner_runtime());
            }
            if let Some(rt) = rt_guard.as_ref() {
                let cfg = self.tuner_config.read().clone();
                rt.start(&cfg)?;
            }
        }

        // CAS to avoid race spawning multiple threads
        // Clone Arc<AtomicBool> locally to avoid capturing &self in spawned thread
        let active_flag = Arc::clone(&self.polling_active);
        if active_flag
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Ok(());
        }

        let app = Arc::new(app);
        let state_ptr = self as *const TunerState as usize;

        let handle = thread::spawn({
            let runtime_ref = Arc::clone(&self.runtime);
            let spectrum_ref = Arc::clone(&self.spectrum_buffer);
            let active_flag = Arc::clone(&active_flag);
            move || {
                log::debug!("Tuner polling thread started (state_ptr={state_ptr:x})");
                while active_flag.load(Ordering::SeqCst) {
                    if let Some(rt) = runtime_ref.read().as_ref() {
                        if let Some(spec) = rt.poll_spectrum() {
                            {
                                *spectrum_ref.write() = Some(spec.clone());
                            }
                            if let Err(e) = app.emit(common::events::tuner::SPECTRUM_DATA, spec) {
                                log::error!("Failed emitting spectrum data: {e}");
                            }
                        }
                    }
                    thread::sleep(Duration::from_millis(20));
                }
                log::debug!("Tuner polling thread stopping (state_ptr={state_ptr:x})");
            }
        });

        *self.polling_thread.write() = Some(handle);

        Ok(())
    }

    /// Stop tuner streaming & polling.
    pub fn stop_tuner_stream(&self) {
        *self.tuning_mode.write() = false;

        // Signal thread to stop
        self.polling_active.store(false, Ordering::SeqCst);

        // Join thread
        if let Some(handle) = self.polling_thread.write().take() {
            let _ = handle.join();
        }

        // Stop runtime
        if let Some(rt) = self.runtime.write().as_ref() {
            rt.stop();
        }
        *self.runtime.write() = None;

        // Clear spectrum
        self.spectrum_buffer.write().take();
    }

    /// Update runtime configuration if runtime is active
    pub fn update_runtime_config(&self, config: &Config) {
        if let Some(runtime) = self.runtime.read().as_ref() {
            runtime.update_config(config);
        }
    }
}
