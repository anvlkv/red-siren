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
    pub key: NodeKey,

    /// Minimum frequency in Hz
    pub min_frequency: f32,

    /// Maximum frequency in Hz
    pub max_frequency: f32,

    /// Minimum magnitude threshold in dB (20*log10)
    pub min_magnitude: f32,

    /// Maximum magnitude threshold in dB (20*log10)
    pub max_magnitude: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Payload representing a snapshot of the frequency spectrum
///
/// Each tuple contains (frequency in Hz, magnitude in dB)
pub struct SpectrumSnapshot(pub Vec<(f32, f32)>);
