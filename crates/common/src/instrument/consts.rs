// Include the generated constants
include!(concat!(env!("OUT_DIR"), "/instrument_constants_gen.rs"));

/// gravitational acceleration in m/s^2 (for buoyancy calculations)
pub const GRAVITY_M_S2: f64 = 9.80665;

// Body mass bounds for acoustic resonator model (high-freq node is lightest)
pub const W_MIN_KG: f64 = 0.02; // mass at highest frequency
pub const W_MAX_KG: f64 = 0.4; // mass at lowest frequency

// Body density range
pub const BODY_DENSITY_MAX_G_CM3: f64 = 800.0;
pub const BODY_DENSITY_MIN_G_CM3: f64 = 2.0;

// Derived volume bounds (V = mass / density)
pub const V_NODE_MAX_CM3: f64 = (W_MAX_KG * 1000.0) / BODY_DENSITY_MIN_G_CM3; // ~200 cm³ at low freq
pub const V_NODE_MIN_CM3: f64 = (W_MIN_KG * 1000.0) / BODY_DENSITY_MAX_G_CM3; // ~0.025 cm³ at high freq

// Acoustic resonator length scale: l_mm = L_MM_ACOUSTIC_SCALE * v_cm3^(1/3)
// Calibrated so l_mm ∈ [~20 mm, ~400 mm] across the node volume range
pub const L_MM_ACOUSTIC_SCALE: f64 = 68.4;

// BPM bounds: low-frequency (large volume) nodes are slow, high-frequency small nodes are fast
pub const BPM_MIN: u16 = 60;
pub const BPM_MAX: u16 = 240;

// Reverb room-size range used for log-scaled mapping from node body volume.
pub const ROOM_SIZE_MIN_M3: f64 = 15.0; // minimum reverb room volume in m³
pub const ROOM_SIZE_MAX_M3: f64 = 60.0; // largest mapped room volume in m³
