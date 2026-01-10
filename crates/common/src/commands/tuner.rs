//! Tuner commands for configuration and spectrum data management

/// Get/emit tuner config (shared name for use_tauri_resource)
pub const CONFIG: &str = "tuner_config";

/// Get/emit tuner layout (shared name for use_tauri_resource)
pub const LAYOUT: &str = "tuner_layout";

/// Get/emit spectrum data (shared name for use_tauri_resource)
pub const SPECTRUM_DATA: &str = "tuner_spectrum_data";

/// Update sensor data
pub const UPDATE_SENSOR: &str = "tuner_update_sensor";

/// Update tuner range
pub const UPDATE_RANGE: &str = "tuner_update_range";

/// Update tuner threshold
pub const UPDATE_THRESHOLD: &str = "tuner_update_threshold";

/// Updae tuner wet ratio
pub const UPDATE_WET_RATIO: &str = "tuner_update_wet_ratio";

/// Reset to defaults from instrument layout
pub const RESET_CONFIG: &str = "tuner_reset_config";

/// Start tuner input stream for spectrum analysis
pub const START_STREAM: &str = "tuner_start_stream";

/// Stop tuner input stream
pub const STOP_STREAM: &str = "tuner_stop_stream";

/// Toggle probe state for preamplified audio
pub const TOGGLE_PROBE: &str = "tuner_toggle_probe";

/// Snapshot input snoop data
pub const INPUT_SNOOP: &str = "tuner_snapshot_input_snoop";
