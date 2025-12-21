use std::collections::BTreeMap;

use common::error::{InstrumentError, Result};
use spectrum_analyzer::{
    samples_fft_to_spectrum, scaling, windows::hamming_window, FrequencyLimit,
};

pub const OUTPUT_ANALYZER_FFT_WINDOW_SIZE: usize = 512;

pub fn analyze(
    window: [f32; OUTPUT_ANALYZER_FFT_WINDOW_SIZE],
    sample_rate: f64,
    min: f32,
    max: f32,
) -> Result<BTreeMap<u32, f32>> {
    let windowed = hamming_window(&window);

    let sampling_rate = sample_rate as u32;

    let spectrum = samples_fft_to_spectrum(
        &windowed,
        sampling_rate,
        FrequencyLimit::Range(min * 0.5, (max * 1.5).min(sampling_rate as f32 / 2.0)),
        Some(&scaling::scale_to_zero_to_one),
    )
    .map_err(|e| InstrumentError::OutputAnalyzerError(e.to_string()))?;

    Ok(spectrum.to_map())
}
