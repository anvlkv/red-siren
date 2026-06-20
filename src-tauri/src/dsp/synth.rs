use crate::audio_runtime::SampleType;

use super::{DspNetwork, DspNetworkBackend};

pub struct Synthesizer {
    // Add fields for the synthesizer state, such as oscillators, filters, etc.
}

impl Synthesizer {
    pub fn new() -> Self {
        Synthesizer {
            // Initialize synthesizer state here
        }
    }

    fn make_backend(&self) -> impl FnMut(&[f32], &mut [f32]) + Send + Sync {
        // simple sine 440 mock

        let mut phase: f32 = 0.0;
        let sample_rate: f32 = 44100.0;
        let frequency: f32 = 440.0;
        let increment: f32 = 2.0 * std::f32::consts::PI * frequency / sample_rate;
        move |input: &[f32], output: &mut [f32]| {
            for sample in output.iter_mut() {
                *sample = (phase).sin();
                phase += increment;
                if phase > 2.0 * std::f32::consts::PI {
                    phase -= 2.0 * std::f32::consts::PI;
                }
            }
        }
    }
}

impl DspNetwork for Synthesizer {
    fn backend(&self) -> Box<dyn DspNetworkBackend + Send + Sync> {
        Box::new(self.make_backend())
    }

    fn set_sample_rate(&self, sample_rate: u32) {
        // Set the sample rate for the synthesizer
    }

    fn set_sample_type(&self, sample_type: SampleType) {
        // Set the sample type for the synthesizer
    }

    fn fade_in(&self, duration: f64) {
        // Implement fade-in logic for the synthesizer
    }

    fn fade_out(&self, duration: f64) {
        // Implement fade-out logic for the synthesizer
    }
}
