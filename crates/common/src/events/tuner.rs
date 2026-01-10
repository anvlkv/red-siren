//! Tuner events for configuration and spectrum data updates

/// Emitted when config changes
pub const CONFIG: &str = "tuner_config";

/// Emitted when layout changes
pub const LAYOUT: &str = "tuner_layout";

/// Emitted with spectrum updates
pub const SPECTRUM_DATA: &str = "tuner_spectrum_data";

/// Reflects an update to tuner constraints
pub const CONSTRAINTS: &str = "tuner_constraints_updated";
