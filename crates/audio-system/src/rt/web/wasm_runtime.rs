//! WASM Audio Worklet Runtime
//!
//! PURPOSE
//! -------
//! Pure WASM compilation target providing DSP processing functions for AudioWorklet.
//! No Tauri dependencies - all communication happens via JSON serialization.
//!
//! ARCHITECTURE
//! ------------
//! - wasm-bindgen entry points for AudioWorkletProcessor integration
//! - JSON-based configuration and control (layout/config from Tauri layer)
//! - Fundsp network management with crossfade semantics
//! - Sample buffer snooping for visualization
//! - Activation source switching (entropy vs microphone)
//!
//! MAYA DRY KISS
//! -------------
//! - Direct port of CPAL DSP functionality to WASM
//! - Clean separation between WASM concerns and Tauri concerns
//! - JSON serialization keeps interface clean and debuggable

use parking_lot::{Mutex, RwLock};
use serde_json;
use std::collections::HashMap;
use wasm_bindgen::prelude::*;

use common::{
    audio::ActivationSource,
    instrument::{Config as InstrumentConfig, Layout as InstrumentLayout},
    tuner::Config as TunerConfig,
    NodeKey,
};
use fundsp::hacker32::prelude::*;

/// Global DSP state managed by the WASM runtime
static DSP_STATE: Mutex<Option<DspState>> = Mutex::new(None);

/// Internal DSP processing state
struct DspState {
    // Audio configuration
    sample_rate: f64,
    channels: usize,
    is_running: bool,
    is_paused: bool,

    // DSP network and controls
    dsp_net: RwLock<Option<Net>>,
    dsp_primary_node_id: RwLock<Option<NodeId>>,
    gain_param: RwLock<Option<Shared>>,

    // Snoop buffers for visualization
    output_snoops: RwLock<HashMap<NodeKey, fundsp::snoop::Snoop>>,
    activation_snoops: RwLock<HashMap<NodeKey, fundsp::snoop::Snoop>>,
    band_controls: RwLock<HashMap<NodeKey, Shared>>,

    // Current activation source
    activation_source: ActivationSource,

    // Last known state for network rebuilds
    last_layout: InstrumentLayout,
    last_config: InstrumentConfig,
    last_tuner_config: TunerConfig,
}

impl DspState {
    fn create_network(&self, config: &InstrumentConfig, tuner_config: &TunerConfig) -> Net {
        // Create network with 1 input, 2 outputs (stereo for now)
        let mut net = Net::new(1, 2);
        net.set_sample_rate(self.sample_rate);

        // Build output system graph & retrieve handles
        let node_handles = crate::create_output_system(config, &mut net, 2);

        let mut siren_controls = HashMap::<NodeKey, Shared>::new();
        let mut band_controls = HashMap::<NodeKey, Shared>::new();

        // Store node handle artifacts (activation/output snoops, control vars)
        {
            let mut activation_snoops = self.activation_snoops.write();
            let mut output_snoops = self.output_snoops.write();
            let mut stored_band_controls = self.band_controls.write();

            activation_snoops.clear();
            output_snoops.clear();
            stored_band_controls.clear();

            for handle in node_handles {
                activation_snoops.insert(handle.key, handle.activation_snoop);
                output_snoops.insert(handle.key, handle.output_snoop);
                siren_controls.insert(handle.key, handle.siren_control);
                band_controls.insert(handle.key, handle.band_control.clone());
                stored_band_controls.insert(handle.key, handle.band_control);
            }
        }

        // Create input system
        crate::create_input_system(tuner_config, &mut net, siren_controls);

        net.allocate();
        log::debug!("Created WASM network: {}", net.display());
        net
    }

    fn update_primary_node(&self, config: &InstrumentConfig, tuner_config: &TunerConfig) {
        let primary_id = match *self.dsp_primary_node_id.read() {
            Some(id) => id,
            None => {
                log::warn!("No primary node id to update");
                return;
            }
        };

        let new_node = self.create_network(config, tuner_config);

        let mut guard = self.dsp_net.write();
        let Some(net) = guard.as_mut() else {
            log::warn!("No DSP network to update");
            return;
        };

        net.crossfade(primary_id, Fade::Smooth, 0.3, Box::new(new_node));
        net.check();
        net.commit();
    }

    fn process_audio_frames(&self, input: &[f32], output: &mut [f32], frame_count: usize) {
        if !self.is_running || self.is_paused {
            // Fill output with silence
            output.fill(0.0);
            return;
        }

        let net_guard = self.dsp_net.read();
        if let Some(net) = net_guard.as_ref() {
            // Clone the net to create a backend for processing
            let mut net_clone = net.clone();
            let mut backend = net_clone.backend();

            // Process frames through the DSP network
            for frame in 0..frame_count {
                let input_start = frame;
                let output_start = frame * self.channels;

                if input_start < input.len() && output_start + self.channels <= output.len() {
                    let in_sample = [input[input_start]];
                    let mut out_sample = [0.0f32; 2];
                    backend.tick(&in_sample, &mut out_sample);

                    output[output_start] = out_sample[0];
                    if self.channels > 1 && output_start + 1 < output.len() {
                        output[output_start + 1] = out_sample[1];
                    }
                }
            }
        } else {
            // No network available, fill with silence or pass-through
            if self.activation_source == ActivationSource::Mic && !input.is_empty() {
                // Pass through microphone input
                for frame in 0..frame_count {
                    let input_start = frame;
                    let output_start = frame * self.channels;

                    if input_start < input.len() && output_start < output.len() {
                        let sample = input[input_start];
                        output[output_start] = sample;
                        if self.channels > 1 && output_start + 1 < output.len() {
                            output[output_start + 1] = sample;
                        }
                    }
                }
            } else {
                // Generate entropy or silence
                output.fill(0.0);
            }
        }
    }
}

/// Initialize DSP state with sample rate and channel count from AudioContext
#[wasm_bindgen]
pub fn wasm_initialize_dsp(sample_rate: f32, channel_count: u32) -> bool {
    log::info!(
        "Initializing DSP: {}Hz, {} channels",
        sample_rate,
        channel_count
    );

    let mut state = DSP_STATE.lock();
    *state = Some(DspState {
        sample_rate: sample_rate as f64,
        channels: channel_count as usize,
        is_running: false,
        is_paused: false,
        dsp_net: RwLock::new(None),
        dsp_primary_node_id: RwLock::new(None),
        gain_param: RwLock::new(None),
        output_snoops: RwLock::new(HashMap::new()),
        activation_snoops: RwLock::new(HashMap::new()),
        band_controls: RwLock::new(HashMap::new()),
        activation_source: ActivationSource::default(),
        last_layout: InstrumentLayout::default(),
        last_config: InstrumentConfig::default(),
        last_tuner_config: TunerConfig::default(),
    });

    true
}

/// Start DSP processing by building Fundsp networks from layout/config JSON
#[wasm_bindgen]
pub fn wasm_start_dsp(
    layout_json: &str,
    config_json: &str,
    tuner_config_json: &str,
    activation_source: u8,
) -> bool {
    log::info!("Starting DSP with activation source: {}", activation_source);

    let mut state_guard = DSP_STATE.lock();
    if let Some(state) = state_guard.as_mut() {
        if state.is_running {
            log::warn!("DSP already running");
            return false;
        }

        // Parse JSON configurations
        let layout: InstrumentLayout = match serde_json::from_str(layout_json) {
            Ok(l) => l,
            Err(e) => {
                log::error!("Failed to parse layout JSON: {}", e);
                return false;
            }
        };

        let config: InstrumentConfig = match serde_json::from_str(config_json) {
            Ok(c) => c,
            Err(e) => {
                log::error!("Failed to parse config JSON: {}", e);
                return false;
            }
        };

        let tuner_config: TunerConfig = match serde_json::from_str(tuner_config_json) {
            Ok(t) => t,
            Err(e) => {
                log::error!("Failed to parse tuner config JSON: {}", e);
                return false;
            }
        };

        // Store configurations for later rebuilds
        state.last_layout = layout;
        state.last_config = config.clone();
        state.last_tuner_config = tuner_config.clone();
        state.activation_source = activation_source.into();

        // Create the DSP network
        let mut net = state.create_network(&config, &tuner_config);

        // Create main gain control
        let gain_param = shared(1.0f32);
        let gain_node = (var(&gain_param) >> follow(0.01)) * pass();
        let primary_id = net.push(Box::new(gain_node));
        *state.gain_param.write() = Some(gain_param.clone());
        *state.dsp_primary_node_id.write() = Some(primary_id);
        *state.dsp_net.write() = Some(net);

        state.is_running = true;
        state.is_paused = false;

        log::info!("DSP started successfully");
        return true;
    }

    log::error!("DSP state not initialized");
    false
}

/// Stop DSP processing and clean up resources
#[wasm_bindgen]
pub fn wasm_stop_dsp() -> bool {
    log::info!("Stopping DSP");

    let mut state_guard = DSP_STATE.lock();
    if let Some(state) = state_guard.as_mut() {
        if !state.is_running {
            return false;
        }

        // Clear all state
        state.is_running = false;
        state.is_paused = false;
        *state.dsp_net.write() = None;
        *state.dsp_primary_node_id.write() = None;
        *state.gain_param.write() = None;
        state.output_snoops.write().clear();
        state.activation_snoops.write().clear();
        state.band_controls.write().clear();

        log::info!("DSP stopped successfully");
        return true;
    }

    false
}

/// Pause DSP processing without destroying state
#[wasm_bindgen]
pub fn wasm_pause_dsp() -> bool {
    log::info!("Pausing DSP");

    let mut state_guard = DSP_STATE.lock();
    if let Some(state) = state_guard.as_mut() {
        if !state.is_running || state.is_paused {
            return false;
        }

        state.is_paused = true;
        return true;
    }

    false
}

/// Resume DSP processing from paused state
#[wasm_bindgen]
pub fn wasm_resume_dsp() -> bool {
    log::info!("Resuming DSP");

    let mut state_guard = DSP_STATE.lock();
    if let Some(state) = state_guard.as_mut() {
        if !state.is_running || !state.is_paused {
            return false;
        }

        state.is_paused = false;
        return true;
    }

    false
}

/// Rebuild Fundsp networks when layout/config changes
#[wasm_bindgen]
pub fn wasm_rebuild_networks(
    _layout_json: &str,
    config_json: &str,
    tuner_config_json: &str,
) -> bool {
    log::info!("Rebuilding DSP networks");

    let state_guard = DSP_STATE.lock();
    if let Some(state) = state_guard.as_ref() {
        if !state.is_running {
            log::warn!("Cannot rebuild networks - DSP not running");
            return false;
        }

        // Parse JSON configurations
        let config: InstrumentConfig = match serde_json::from_str(config_json) {
            Ok(c) => c,
            Err(e) => {
                log::error!("Failed to parse config JSON for rebuild: {}", e);
                return false;
            }
        };

        let tuner_config: TunerConfig = match serde_json::from_str(tuner_config_json) {
            Ok(t) => t,
            Err(e) => {
                log::error!("Failed to parse tuner config JSON for rebuild: {}", e);
                return false;
            }
        };

        // Update networks with crossfade
        state.update_primary_node(&config, &tuner_config);

        log::info!("DSP networks rebuilt successfully");
        return true;
    }

    false
}

/// Switch activation source between entropy and microphone input
#[wasm_bindgen]
pub fn wasm_set_activation_source(source: u8) -> bool {
    log::info!("Setting activation source: {}", source);

    let mut state_guard = DSP_STATE.lock();
    if let Some(state) = state_guard.as_mut() {
        state.activation_source = source.into();

        // Rebuild networks if running
        if state.is_running {
            state.update_primary_node(&state.last_config.clone(), &state.last_tuner_config.clone());
        }

        return true;
    }

    false
}

/// Update band control parameters for specific nodes
#[wasm_bindgen]
pub fn wasm_set_band_control(group: u8, key: u8, value: f32) -> bool {
    log::debug!(
        "Setting band control: group={}, key={}, value={}",
        group,
        key,
        value
    );

    let state_guard = DSP_STATE.lock();
    if let Some(state) = state_guard.as_ref() {
        let node_key = NodeKey(group, key);
        let band_controls = state.band_controls.read();

        if let Some(control) = band_controls.get(&node_key) {
            control.set(value);
            return true;
        } else {
            log::warn!("Band control not found for group={}, key={}", group, key);
        }
    }

    false
}

/// Process audio frames by calling Fundsp networks, fill output buffer from input
#[wasm_bindgen]
pub unsafe fn wasm_process_audio(
    input_ptr: *const f32,
    output_ptr: *mut f32,
    frame_count: u32,
) -> bool {
    let state_guard = DSP_STATE.lock();
    if let Some(state) = state_guard.as_ref() {
        let frames = frame_count as usize;
        let input_samples = frames; // mono input
        let output_samples = frames * state.channels;

        let input_slice = unsafe { std::slice::from_raw_parts(input_ptr, input_samples) };
        let output_slice = unsafe { std::slice::from_raw_parts_mut(output_ptr, output_samples) };

        // Process all frames at once
        state.process_audio_frames(input_slice, output_slice, frames);

        return true;
    }

    false
}

/// Return current sample buffer from output snoop for specific node
#[wasm_bindgen]
pub fn wasm_get_output_snoop(group: u8, key: u8) -> Box<[f32]> {
    let state_guard = DSP_STATE.lock();
    if let Some(state) = state_guard.as_ref() {
        let node_key = NodeKey(group, key);
        let output_snoops = state.output_snoops.read();

        if let Some(snoop) = output_snoops.get(&node_key) {
            let mut samples = Vec::with_capacity(snoop.capacity());
            for i in (0..snoop.capacity()).rev() {
                samples.push(snoop.at(i));
            }
            return samples.into_boxed_slice();
        }
    }

    // Return empty buffer if not found
    Vec::new().into_boxed_slice()
}

/// Return JSON array of all output snoop samples with group/key identifiers
#[wasm_bindgen]
pub fn wasm_get_all_output_snoops() -> String {
    let state_guard = DSP_STATE.lock();
    if let Some(state) = state_guard.as_ref() {
        let output_snoops = state.output_snoops.read();
        let mut result = Vec::new();

        for (node_key, snoop) in output_snoops.iter() {
            let mut samples = Vec::with_capacity(snoop.capacity());
            for i in (0..snoop.capacity()).rev() {
                samples.push(snoop.at(i));
            }
            let snoop_data = serde_json::json!({
                "group": node_key.0,
                "key": node_key.1,
                "samples": samples
            });
            result.push(snoop_data);
        }

        return serde_json::to_string(&result).unwrap_or_else(|_| "[]".to_string());
    }

    "[]".to_string()
}

/// Return current sample buffer from activation snoop for specific node
#[wasm_bindgen]
pub fn wasm_get_activation_snoop(group: u8, key: u8) -> Box<[f32]> {
    let state_guard = DSP_STATE.lock();
    if let Some(state) = state_guard.as_ref() {
        let node_key = NodeKey(group, key);
        let activation_snoops = state.activation_snoops.read();

        if let Some(snoop) = activation_snoops.get(&node_key) {
            let mut samples = Vec::with_capacity(snoop.capacity());
            for i in (0..snoop.capacity()).rev() {
                samples.push(snoop.at(i));
            }
            return samples.into_boxed_slice();
        }
    }

    // Return empty buffer if not found
    Vec::new().into_boxed_slice()
}

/// Return JSON array of all activation snoop samples with group/key identifiers
#[wasm_bindgen]
pub fn wasm_get_all_activation_snoops() -> String {
    let state_guard = DSP_STATE.lock();
    if let Some(state) = state_guard.as_ref() {
        let activation_snoops = state.activation_snoops.read();
        let mut result = Vec::new();

        for (node_key, snoop) in activation_snoops.iter() {
            let mut samples = Vec::with_capacity(snoop.capacity());
            for i in (0..snoop.capacity()).rev() {
                samples.push(snoop.at(i));
            }
            let snoop_data = serde_json::json!({
                "group": node_key.0,
                "key": node_key.1,
                "samples": samples
            });
            result.push(snoop_data);
        }

        return serde_json::to_string(&result).unwrap_or_else(|_| "[]".to_string());
    }

    "[]".to_string()
}

/// Initialize panic hook for better WASM debugging
#[wasm_bindgen(start)]
pub fn wasm_init() {
    console_error_panic_hook::set_once();

    log::info!("WASM Audio Runtime initialized");
}
