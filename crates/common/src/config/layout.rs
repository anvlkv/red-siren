use std::num::NonZero;

use nalgebra::geometry::Point2;
use serde::{Deserialize, Serialize};

use crate::{safe_area::SafeArea, Line, Rect};

pub const LAYOUT_PRIMES: const_primes::Primes<20> = const_primes::Primes::new();
pub const MAX_LP: usize = LAYOUT_PRIMES.as_array()[19] as usize; // 71

#[derive(Clone, Copy, Serialize, Deserialize, Debug)]
pub struct Layout {
    /// Number of keys and bands in each band
    pub num_keys_per_band: NonZero<u8>,
    /// Number of bands
    pub num_bands: NonZero<u8>,
    /// Channel of the first band in the layout
    pub first_band_channel: super::BandChannel,
    /// Whether `Horizontal` -> main X OR `Vertical` -> main Y layout is used
    pub orientation: super::LayoutOrientation,
    /// Start and end positions of **left** channel string
    pub left_string_position: Line,
    /// Start and end positions of **right** channel string
    pub right_string_position: Line,
    /// Distance between strings
    pub instrument_breadth: f64,
    /// Radius of each key
    pub key_radius: f64,
    /// Length of the `track` of key's band
    pub key_band_length: f64,
    /// Breadth of the `track` of key's band
    pub key_band_breadth: f64,
    /// Distance between `track`s (keys) inside a band (main axis)
    pub key_bands_gap: f64,
    /// Distance between bands (main axis)
    pub bands_gap: f64,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            num_keys_per_band: NonZero::new(3).unwrap(),
            num_bands: NonZero::new(2).unwrap(),
            first_band_channel: super::BandChannel::Left,
            orientation: super::LayoutOrientation::Horizontal,
            left_string_position: (Point2::new(0, 0), Point2::new(1, 0)),
            right_string_position: (Point2::new(0, 1), Point2::new(1, 1)),
            instrument_breadth: 0.5,
            key_radius: 0.05,
            key_band_length: 0.4,
            key_band_breadth: 0.1,
            key_bands_gap: 0.05,
            bands_gap: 0.1,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct LayoutCandidate {
    layout: Layout,
    #[allow(dead_code)]
    score: f64,
}

impl LayoutCandidate {
    fn generate_candidates(_screen_estate: &Rect, _safe_area: &SafeArea) -> Vec<Self> {
        todo!()
    }

    fn evaluate(
        _candidates: &mut [Self],
        _screen_estate: &Rect,
        _safe_area: &SafeArea,
    ) -> Option<Self> {
        todo!()
    }
}

impl Layout {
    pub fn new(screen_estate: &Rect, safe_area: &SafeArea) -> Self {
        let mut candidates = LayoutCandidate::generate_candidates(screen_estate, safe_area);
        let best_candidate =
            LayoutCandidate::evaluate(candidates.as_mut_slice(), screen_estate, safe_area);
        best_candidate.map(|c| c.layout).unwrap_or_default()
    }
}
