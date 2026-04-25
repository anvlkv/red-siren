// Include the generated constants
include!(concat!(env!("OUT_DIR"), "/instrument_constants_gen.rs"));

/// gravitational acceleration in m/s^2 (for buoyancy calculations)
pub const GRAVITY_M_S2: f64 = 9.80665;
