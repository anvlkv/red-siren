use serde::{Deserialize, Serialize};

#[cfg(feature = "egui")]
use enum2egui::GuiInspect;

#[derive(Clone, Copy, Serialize, Deserialize, Debug)]
#[cfg_attr(feature = "egui", derive(enum2egui::Gui))]
pub struct Node {
    #[cfg_attr(feature = "egui", enum2egui(skip))]
    pub key: NodeKey,
    pub base_frequency_hz: f64,
    pub path_spacing_hz: f64,
    pub num_modes: usize,
    pub mode_spacing_hz: f64,
    pub mode_decay_s: f64,
}

#[derive(Clone, Copy, Serialize, Deserialize, Debug, PartialEq, Eq, Hash)]
pub struct NodeKey {
    pub key: usize,
    pub band_key: usize,
}

impl Node {
    const LN_1000: f64 = 6.907_755_278_982_137;
    const F_REF: f64 = 500.0;

    fn bandwidth_hz_up_until_mode(&self, mode_index: usize) -> f64 {
        (self.mode_spacing_hz * (mode_index as f64 + 1.0)).max(0.01) // Avoid zero bandwidth.
    }

    /// Calculate the frequency of a given path and mode. Path index 0 is left, 1 is center, 2 is right.
    pub fn path_mode_frequency_hz(&self, path_index: usize, mode_index: usize) -> f64 {
        let fundamental = self.base_frequency_hz;
        // Center path (index 1) has no offset, left path (index 0) is negative, right path (index 2) is positive.
        let path_offset = self.path_spacing_hz * (path_index as f64 - 1.0);
        let mode_offset = self.mode_spacing_hz * mode_index as f64;
        fundamental + path_offset + mode_offset
    }

    /// Calculate the Q factor for a given path and mode, incorporating the mode decay time.
    pub fn path_mode_q(&self, path_index: usize, mode_index: usize) -> f64 {
        let f_hz = self.path_mode_frequency_hz(path_index, mode_index).max(1.0);

        let t60_ref_s = self.mode_decay_s; // reinterpret this field as T60 at f_ref
        let alpha = -0.25;

        let t60_s = t60_ref_s * (f_hz / Self::F_REF).powf(alpha);
        let q_decay = std::f64::consts::PI * f_hz * t60_s / Self::LN_1000;

        let mode_bandwidth_hz = self.bandwidth_hz_up_until_mode(mode_index).max(0.2);
        let q_bw = f_hz / mode_bandwidth_hz;

        let q_total = 0.5 * (q_decay + q_bw);
        q_total.clamp(120.0, 8000.0)
    }

    /// Calculate the gain for a given mode, which could be used to scale the output of the resonator.
    pub fn mode_gain_db(&self, mode_index: usize) -> f64 {
        let f_hz = self.path_mode_frequency_hz(1, mode_index).max(20.0);

        // Chosen bell voicing defaults.
        let spectral_tilt = 0.35; // smaller = brighter
        let mode_falloff = 0.25; // smaller = less attenuation across modes
        let makeup_db = 14.0; // global loudness lift

        let tilt = (Self::F_REF / f_hz).powf(spectral_tilt);
        let modal = 1.0 / (mode_index as f64 + 1.0).powf(mode_falloff);

        let gain_linear = tilt * modal;
        (20.0 * gain_linear.log10() + makeup_db).clamp(-24.0, 6.0)
    }
}
