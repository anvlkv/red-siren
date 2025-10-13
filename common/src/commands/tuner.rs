//! Tuner commands for configuration and spectrum data management

/// Get/emit tuner config (shared name for use_tauri_resource)
pub const CONFIG: &str = "tuner_config";

/// Get/emit spectrum data (shared name for use_tauri_resource)
pub const SPECTRUM_DATA: &str = "tuner_spectrum_data";

/// Update sensor data
pub const UPDATE_SENSOR: &str = "tuner_update_sensor";

/// Reset to defaults from instrument layout
pub const RESET_CONFIG: &str = "tuner_reset_config";

/// Start tuner input stream for spectrum analysis
pub const START_STREAM: &str = "tuner_start_stream";

/// Stop tuner input stream
pub const STOP_STREAM: &str = "tuner_stop_stream";
