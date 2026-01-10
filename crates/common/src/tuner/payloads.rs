//! Payload types for tuner commands and events

use serde::{Deserialize, Serialize};

use crate::NodeKey;

/// Spectrum data from FFT analysis
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpectrumData {
    /// Current frame spectrum values in dB (20*log10)
    pub current_magnitudes: Vec<f32>,

    /// Max hold spectrum values in dB (20*log10)
    pub max_magnitudes: Vec<f32>,

    /// Sensor excitement levels from FFT analyzer (0-1)
    pub sensor_excitements: Vec<f32>,

    /// Max-hold sensor excitement levels (0-1)
    pub max_excitements: Vec<f32>,

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
    pub keys: Vec<NodeKey>,

    /// Minimum frequency in Hz
    pub min_frequency_increment: f32,

    /// Maximum frequency in Hz
    pub max_frequency_increment: f32,

    /// Minimum magnitude threshold in 0..1
    pub min_magnitude_increment: f32,

    /// Maximum magnitude threshold in 0..1
    pub max_magnitude_increment: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Payload representing a snapshot of the frequency spectrum
///
/// Each tuple contains (frequency in Hz, magnitude in dB)
pub struct SpectrumSnapshot(pub Vec<(f32, f32)>);

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Payload for updating the tuner's frequency range
pub struct UpdateRangePayload {
    pub min_frequency: Option<f32>,
    pub max_frequency: Option<f32>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Payload for updating the NY compression threshold
pub struct UpdateNyThresholdPayload {
    pub ny_threshold: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Payload for updating the NY wet ratio
pub struct UpdateWetRatioPayload {
    pub wet_ratio: f32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
/// Reflects the current tuner constraints
pub struct ReflectTunerConstraints {
    pub min_frequency: Option<f32>,
    pub max_frequency: Option<f32>,
    pub frequency_limit: (f32, f32),
    pub ny_threshold: f32,
    pub wet_ratio: f32,
}
