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

        Box::new(
            move |tick_data: &super::NetTickData, _input: &[S], output: &mut [S]| {
                energy.process(tick_data, &[], &mut energy_output);
                chamber_input[0] = energy_output[0];
                chamber_input[1] = chamber_output[1];
                chamber_input[2..].fill(S::zero());

                chamber_output.fill(S::zero());

                for chamber in chambers.iter_mut() {
                    chamber.process(tick_data, &chamber_input, &mut chamber_output);
                    chamber_input.copy_from_slice(&chamber_output);
                }

                // centered signal output
                // output[0] =
                //     ((chamber_output[2] - S::from(1.0).unwrap()) / S::from(2.0).unwrap()).tanh();
                // output[1] =
                //     ((chamber_output[3] - S::from(1.0).unwrap()) / S::from(2.0).unwrap()).tanh();

                output[0] = chamber_output[2];
                output[1] = chamber_output[3];
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

// impl Siren {
//     pub fn new(chambers: Vec<chamber::Chamber>) -> Self {
//         Self {
//             energy: Arc::new(energy::EnergySource::new(chambers.len() as f32)),
//             chambers: Arc::new(chambers),
//         }
//     }

//     pub fn generate<S: Float>(&self, output: &mut [S], sample_rate: u32) {
//         let mut generator_output = [S::zero()];
//         self.energy.generate(&mut generator_output, sample_rate);
//         let num_chambers = self.chambers.len();
//         let [_, left, right, _] = self.chambers.iter().enumerate().fold(
//             [generator_output[0], S::zero(), S::zero(), S::zero()],
//             |[energy_xct, prev_left, prev_right, prev_adj_xct], (at, chamber)| {
//                 let [output_xct, remainder_xct, output_adj_xct] =
//                     chamber.process([energy_xct, prev_adj_xct]);
//                 let gain = S::from(num_chambers - at)
//                     .map(|s| S::one() / s)
//                     .unwrap_or(S::zero());

//                 [
//                     output_xct,
//                     if chamber.is_left_channel {
//                         output_xct * gain + prev_left
//                     } else {
//                         remainder_xct * gain + prev_left
//                     },
//                     if chamber.is_left_channel {
//                         remainder_xct * gain + prev_right
//                     } else {
//                         output_xct * gain + prev_right
//                     },
//                     output_adj_xct,
//                 ]
//             },
//         );
//         output[0] = left;
//         output[1] = right;
//     }
// }
