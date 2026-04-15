pub mod adsr;
pub mod excitor;
pub mod input;
pub mod metro;
pub mod output_analyzer;
pub mod values;

#[cfg(feature = "editor")]
use values::FineTunedValues;

use std::collections::HashMap;

use crate::rt::ExcitementSource;
use common::NodeKey;
use fundsp::{
    net::Net,
    prelude::{pass, shared, var, An, AudioUnit},
    shared::Shared,
    snoop::{Snoop, SnoopBackend},
    Float, Real,
};

pub use excitor::control::Control as ExcitementControl;

pub struct NodeHandles {
    pub key: NodeKey,
    pub excitement_snoop: Snoop,
    pub secondary_excitement_snoop: Snoop,
    pub output_snoop: Snoop,
    pub siren_control: ExcitementControl,
    pub band_control: Shared,
    pub key_control: Shared,
}

pub struct SensorHandles {
    pub key: NodeKey,
    pub min_frequency: Shared,
    pub max_frequency: Shared,
    pub min_magnitude: Shared,
    pub max_magnitude: Shared,
}

/// Create output subsystem for live playback.
#[must_use]
pub fn mount_output_system<S: Real + Float + 'static>(
    _config: &common::instrument::Config,
    _net: &mut Net,
    _num_channels: usize,
    #[cfg(feature = "editor")] _values: &FineTunedValues,
) -> Vec<NodeHandles> {
    vec![]
}

/// Build and wire the input (tuner) sub-graph into `net`.
///
/// Returns one [`SensorHandles`] per entry in `config.sensor_data`.
/// The caller owns the [`Shared`] values inside each handle and may update
/// them at any time to change per-sensor frequency / magnitude bounds.
///
/// ## Input layout expected by the graph
///
/// ```text
/// net input 0                      – audio in
/// net input 1 + i*4 + 0            – sensor[i] min_frequency
/// net input 1 + i*4 + 1            – sensor[i] max_frequency
/// net input 1 + i*4 + 2            – sensor[i] min_magnitude
/// net input 1 + i*4 + 3            – sensor[i] max_magnitude
/// net input 1 + n*4                – global min_frequency
/// net input 1 + n*4 + 1            – global max_frequency
/// ```
///
/// (Only the `Mic` source wires all of those; `Entropy` and `Manual` only
/// connect audio input 0 and ignore the rest.)
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn create_input_system<S: Real + Float + 'static>(
    config: &common::tuner::Config,
    net: &mut Net,
    excitements: HashMap<NodeKey, ExcitementControl>,
    source: ExcitementSource,
    spectrum_thb: &excitor::SpectrumBuffer,
    input_snoop: Option<An<SnoopBackend>>,
    freq_range: (&Shared, &Shared),
    ny_params: (&Shared, &Shared),
    _tap_channel: usize,
) -> Vec<SensorHandles> {
    use input::analyzer::FFTAnalyzer;
    use input::preamp::create_sensors_preamp;

    // Build per-sensor SensorHandles from the tuner config.
    let sensor_handles: Vec<SensorHandles> = config
        .sensor_data
        .iter()
        .map(|s| SensorHandles {
            key: s.key,
            min_frequency: shared(s.min_frequency),
            max_frequency: shared(s.max_frequency),
            min_magnitude: shared(s.min_magnitude),
            max_magnitude: shared(s.max_magnitude),
        })
        .collect();

    match source {
        ExcitementSource::Mic => {
            // Build inner net: optional snoop tap → calibration preamp.
            let inner: Box<dyn AudioUnit> = {
                let preamp = create_sensors_preamp::<S>(var(ny_params.0), var(ny_params.1));
                if let Some(snoop_be) = input_snoop {
                    // audio in → SnoopBackend (records + passes through) → preamp → audio out
                    let mut inner_net = Net::new(1, 1);
                    let s_id = inner_net.push(Box::new(snoop_be));
                    let p_id = inner_net.push(Box::new(preamp));
                    inner_net.connect_input(0, s_id, 0);
                    inner_net.connect(s_id, 0, p_id, 0);
                    inner_net.pipe_output(p_id);
                    Box::new(inner_net)
                } else {
                    Box::new(preamp)
                }
            };

            let analyzer =
                FFTAnalyzer::<S>::new(inner, config.clone(), excitements, spectrum_thb.clone());

            let n = sensor_handles.len();
            let analyzer_id = net.push(Box::new(analyzer));

            // Wire audio input → analyzer input 0.
            net.connect_input(0, analyzer_id, 0);

            // Wire per-sensor shared values into analyzer inputs (4 per sensor).
            for (i, sh) in sensor_handles.iter().enumerate() {
                let base = 1 + i * 4;
                let min_f_id = net.push(Box::new(var(&sh.min_frequency)));
                let max_f_id = net.push(Box::new(var(&sh.max_frequency)));
                let min_m_id = net.push(Box::new(var(&sh.min_magnitude)));
                let max_m_id = net.push(Box::new(var(&sh.max_magnitude)));
                net.connect(min_f_id, 0, analyzer_id, base);
                net.connect(max_f_id, 0, analyzer_id, base + 1);
                net.connect(min_m_id, 0, analyzer_id, base + 2);
                net.connect(max_m_id, 0, analyzer_id, base + 3);
            }

            // Wire global frequency range (last 2 analyzer inputs).
            let limit_base = 1 + n * 4;
            let min_r_id = net.push(Box::new(var(freq_range.0)));
            let max_r_id = net.push(Box::new(var(freq_range.1)));
            net.connect(min_r_id, 0, analyzer_id, limit_base);
            net.connect(max_r_id, 0, analyzer_id, limit_base + 1);

            net.pipe_output(analyzer_id);
        }

        ExcitementSource::Entropy => {
            // Random-excitement driver: passes audio through while periodically
            // setting every control to a fresh LCG-derived (re, im) pair.
            let driver =
                excitor::entropy::EntropyDriver::new(excitements, config.sample_rate as f64);
            let id = net.push(Box::new(driver));
            net.connect_input(0, id, 0);
            net.pipe_output(id);
        }

        ExcitementSource::Manual => {
            // Excitement is driven externally via the UI; just pass audio through.
            let id = net.push(Box::new(pass()));
            net.connect_input(0, id, 0);
            net.pipe_output(id);
        }
    }

    sensor_handles
}

#[cfg(test)]
mod tests;
