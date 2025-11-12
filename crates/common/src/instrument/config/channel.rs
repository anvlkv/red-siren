use serde::{Deserialize, Serialize};

use crate::instrument::consts::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
/// Output chanel of the group
pub enum GroupChannel {
    Left,
    Right,
}

fn fundamental_frequency(n: usize, v: f64, l: f64) -> f64 {
    (n as f64 * v) / (2.0 * l)
}

impl GroupChannel {
    pub(crate) fn from_keys_groups(k: u32, g: u32) -> Self {
        match (g.is_multiple_of(2), k.is_multiple_of(2)) {
            (false, false) => Self::Left,
            (false, true) => Self::Right,
            (true, false) => Self::Right,
            (true, true) => Self::Left,
        }
    }

    pub fn nth_channel_from_first(&self, n: usize) -> Self {
        if n.is_multiple_of(2) {
            *self
        } else {
            match self {
                Self::Left => Self::Right,
                Self::Right => Self::Left,
            }
        }
    }

    pub(crate) fn compute_fundamentals(&self, l: f64, mut n_base: usize) -> (f64, usize) {
        let v = match self {
            Self::Left => CRIMSON_RED_WAVESPEED,
            Self::Right => CINNABAR_RED_WAVESPEED,
        };

        let mut f: f64 = 0.0;

        while f < SOFT_MIN_FREQ_HZ {
            f = fundamental_frequency(n_base, v, l);
            if f < SOFT_MIN_FREQ_HZ {
                n_base += 1;
            }
        }

        (f, n_base)
    }
}
