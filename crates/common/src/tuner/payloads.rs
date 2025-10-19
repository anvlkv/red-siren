//! Payload types for tuner commands and events

use serde::{Deserialize, Serialize};

/// Spectrum data from FFT analysis
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpectrumData {
    /// Current frame spectrum values in dB (20*log10)
    pub current_magnitudes: Vec<f32>,

    /// Max hold spectrum values in dB (20*log10)
    pub max_magnitudes: Vec<f32>,

    /// Sensor activation levels from FFT analyzer (0-1)
    pub sensor_activations: Vec<f32>,

    /// Max-hold sensor activation levels (0-1)
    pub max_activations: Vec<f32>,

    /// Corresponding frequencies for each magnitude
    pub frequencies: Vec<f32>,

    /// Sample rate in Hz
    pub sample_rate: f32,

    /// FFT window size
    pub fft_size: usize,
}

/// Payload for updating a sensor's configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSensorPayload {
    /// Index of the sensor to update
    pub index: usize,

    /// Minimum frequency in Hz
    pub min_frequency: f32,

    /// Maximum frequency in Hz
    pub max_frequency: f32,

    /// Minimum magnitude threshold in dB (20*log10)
    pub min_magnitude: f32,

    /// Maximum magnitude threshold in dB (20*log10)
    pub max_magnitude: f32,
}
