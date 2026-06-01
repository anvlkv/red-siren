use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Band {
    pub channel: super::BandChannel,
    pub medium_hf_loss: f64,
    pub medium_radiation_scale: f64,
    pub medium_q_scale: f64,
}

// impl Band {
//     pub fn gain(&self, frequency_hz: f64, coupling: f64) -> f64 {
//         let hf_loss = self.medium_hf_loss * (frequency_hz / 1000.0).max(1.0).log10();
//         let radiation_loss = self.medium_radiation_scale * (frequency_hz / 1000.0).max(1.0).log10();
//         let coupling_loss = 20.0 * (1.0 - coupling.clamp(0.0, 1.0)).log10();
//         10f64.powf(-0.05 * (hf_loss + radiation_loss + coupling_loss))
//     }
// }
