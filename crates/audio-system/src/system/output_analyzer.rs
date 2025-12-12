use std::collections::BTreeMap;

use common::{
    error::{InstrumentError, Result},
    instrument::consts,
};
use spectrum_analyzer::{samples_fft_to_spectrum, scaling, windows::hann_window, FrequencyLimit};

pub const OUTPUT_ANALYZER_FFT_WINDOW_SIZE: usize = 64;

pub fn analyze(
    window: [f32; OUTPUT_ANALYZER_FFT_WINDOW_SIZE],
    sample_rate: f64,
) -> Result<BTreeMap<u32, f32>> {
    let windowed = hann_window(&window);

    let sampling_rate = sample_rate as u32;

    // let scaling = scaling::combined(&[&scaling::divide_by_N, &scaling::scale_to_zero_to_one]);

    let spectrum = samples_fft_to_spectrum(
        &windowed,
        sampling_rate,
        FrequencyLimit::Range(
            consts::SOFT_MIN_FREQ_HZ as f32,
            consts::SOFT_MAX_FREQ_HZ as f32,
        ),
        Some(&scaling::scale_to_zero_to_one),
    )
    .map_err(|e| InstrumentError::OutputAnalyzerError(e.to_string()))?;

    Ok(spectrum.to_map())
}
