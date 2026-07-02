mod chamber;
mod config;
mod energy;

use num_traits::Float;

use crate::dsp::DspUnit;

pub use chamber::*;
pub use config::*;

pub struct Siren {
    pub energy: energy::EnergySource,
    pub chambers: Vec<chamber::Chamber>,
}

impl Siren {
    pub fn new(config: SirenConfig) -> Self {
        let chambers = config.chambers();
        let energy = energy::EnergySource::new(0.2);

        Self { energy, chambers }
    }
}

impl DspUnit for Siren {
    fn backend<S: Float + Send + Sync + 'static>(
        &self,
    ) -> Box<dyn super::DspUnitBackend<S> + Send + Sync> {
        let mut energy = self.energy.backend::<S>();
        let mut chambers = self
            .chambers
            .iter()
            .map(|c| c.backend::<S>())
            .collect::<Vec<_>>();
        let mut energy_output = energy::EnergySource::output_frame::<S>();
        let mut chamber_input = chamber::Chamber::input_frame::<S>();
        let mut chamber_output = chamber::Chamber::output_frame::<S>();

        let mut l_peak = S::one();
        let mut r_peak = S::one();
        let release = S::one() - S::epsilon().sqrt();

        Box::new(
            move |tick_data: &super::NetTickData, _input: &[S], output: &mut [S]| {
                energy.process(tick_data, &[], &mut energy_output);
                chamber_input[0] = energy_output[0];
                chamber_input[1] = chamber_output[1];
                chamber_input[2..].fill(S::zero());

                chamber_output.fill(S::zero());

                l_peak = l_peak * release;
                r_peak = r_peak * release;

                for chamber in chambers.iter_mut() {
                    chamber.process(tick_data, &chamber_input, &mut chamber_output);
                    chamber_input.copy_from_slice(&chamber_output);
                    l_peak = l_peak.max(chamber_output[2].abs());
                    r_peak = r_peak.max(chamber_output[3].abs());
                }

                // centered signal output
                output[0] = centered(chamber_output[2], l_peak);
                output[1] = centered(chamber_output[3], r_peak);
            },
        )
    }

    fn output_frame<S: Float + Send + Sync + 'static>() -> Vec<S> {
        vec![S::zero(); 2]
    }

    fn input_frame<S: Float + Send + Sync + 'static>() -> Vec<S> {
        vec![]
    }
}

fn centered<S: Float>(value: S, peak: S) -> S {
    if peak == S::zero() {
        return S::zero();
    }
    let normalized = value / peak;
    normalized * S::from(2.0).unwrap() - S::one()
}
