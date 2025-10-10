use mint::{Point2, Vector2};
use num_complex::Complex32;

use crate::{
    instrument::consts::{MAX_FREQ_HZ, MIN_FREQ_HZ},
    tuner::{config::gain_linear, layout::Layout},
};

/// Represents the full tuner data set (layout + sensors + FFT mapping).
#[derive(Debug, PartialEq, Clone, Default)]
pub struct TunerData {
    pub layout: Layout,
    pub sensor_data: Vec<SensorData>,
}

#[derive(Debug, PartialEq, Clone, Copy, Default)]
pub struct SensorData(pub Complex32, pub Complex32);
