use std::num::NonZero;

use mint::{Point2, Vector2};

use crate::{
    orientation::LayoutOrientation,
    safe_area::{self, SafeArea, DEFAULT_SAFE_AREA},
};

/// Line between two points
pub type Line = (Point2<f32>, Point2<f32>);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Layout {
    /// total screen estate available to layout the instrument
    pub space: Vector2<f32>,
    /// whether `Horizontal` or `Vertical` layout is used,
    /// determines the main and auxilary axes of the instrument layout
    pub orientation: LayoutOrientation,
    /// start and end postions of **left** channel string
    pub left_string_position: Line,
    /// start and end postions of **right** channel string
    pub right_string_position: Line,
    /// radius of each key
    pub key_radius: f32,
    /// length of the `track` of key's band
    pub key_band_length: f32,
    /// breadth of the `track` of key's band
    pub key_band_breadth: f32,
    /// minimum distance from edge of the screen to any interactive element
    pub safe_area_padding: SafeArea,
    /// distance between `track`s in one group,
    /// along the main axis of instrument
    pub key_bands_gap: f32,
    /// distance between `group`s,
    /// along the main axis of instrument
    pub groups_gap: f32,
    /// number of keys and bands in each group
    pub num_keys_per_group: NonZero<u8>,
    /// number of groups
    pub num_groups: NonZero<u8>,
}

impl Eq for Layout {}

/// 54 first primes - up to 251
const LAYOUT_PRIMES: const_primes::Primes<54> = const_primes::Primes::new();
/// minimum radius of each key
const MIN_KEY_RADIUS: f32 = 16.0;
/// minimum padding of a key in a band
const MIN_BAND_PADDING: f32 = 8.0;
/// minimum gap between bands and groups
const MIN_GAP: f32 = 16.0;
/// maximum ratio of key radius to band breadth
const MAX_KEY_RADIUS_TO_BREADTH_RATIO: f32 = 0.4;

struct LayoutCandidate {
    key_radius: f32,
    key_band_breadth: f32,
    key_bands_gap: f32,
    groups_gap: f32,
    num_keys_per_group: u32,
    num_groups: u32,
}

impl LayoutCandidate {
    fn valid(&self, instrument_breadth: f32) -> bool {
        self.num_groups > 0
            && self.num_groups <= u8::MAX as u32
            && self.num_keys_per_group > 0
            && self.num_keys_per_group <= u8::MAX as u32
            && self.key_radius >= MIN_KEY_RADIUS
            && self.key_band_breadth >= (self.key_radius * 2.0 + MIN_BAND_PADDING)
            && self.key_bands_gap >= MIN_GAP
            && self.groups_gap >= MIN_GAP
            && (self.key_radius / instrument_breadth) <= MAX_KEY_RADIUS_TO_BREADTH_RATIO
    }

    fn fallback(length: f32, instrument_breadth: f32) -> Self {
        let total_keys = 12.0;
        let key_radius = (length / total_keys) / 2.0;
        let key_band_breadth = instrument_breadth / 3.0;
        let key_bands_gap = (length - total_keys * key_radius * 2.0) / (total_keys + 1.0);
        let groups_gap = if total_keys > 1.0 {
            (length - total_keys * key_radius * 2.0 - (total_keys - 1.0) * MIN_GAP) / total_keys
        } else {
            length - total_keys * key_radius * 2.0
        };

        LayoutCandidate {
            key_radius,
            key_band_breadth,
            key_bands_gap,
            groups_gap,
            num_keys_per_group: 12,
            num_groups: 1,
        }
    }
}

impl Layout {
    pub fn from_screen_estate(space: Vector2<f32>) -> Self {
        Self::compute(
            space,
            DEFAULT_SAFE_AREA,
            DEFAULT_SAFE_AREA,
            DEFAULT_SAFE_AREA,
            DEFAULT_SAFE_AREA,
        )
        .unwrap_or_else(|| {
            Self::fallback(
                space,
                DEFAULT_SAFE_AREA,
                DEFAULT_SAFE_AREA,
                DEFAULT_SAFE_AREA,
                DEFAULT_SAFE_AREA,
            )
        })
    }

    pub fn with_safe_area(
        self,
        top_safe_area: f32,
        right_safe_area: f32,
        bottom_safe_area: f32,
        left_safe_area: f32,
    ) -> Self {
        Self::compute(
            self.space,
            top_safe_area,
            right_safe_area,
            bottom_safe_area,
            left_safe_area,
        )
        .unwrap_or_else(|| {
            Self::fallback(
                self.space,
                top_safe_area,
                right_safe_area,
                bottom_safe_area,
                left_safe_area,
            )
        })
    }

    pub fn from_screen_estate_with_safe_area(
        space: Vector2<f32>,
        top_safe_area: f32,
        right_safe_area: f32,
        bottom_safe_area: f32,
        left_safe_area: f32,
    ) -> Self {
        Self::compute(
            space,
            top_safe_area,
            right_safe_area,
            bottom_safe_area,
            left_safe_area,
        )
        .unwrap_or_else(|| {
            Self::fallback(
                space,
                top_safe_area,
                right_safe_area,
                bottom_safe_area,
                left_safe_area,
            )
        })
    }

    fn string_positions(
        orientation: LayoutOrientation,
        breadth: f32,
        length: f32,
        instrument_breadth: f32,
    ) -> (Line, Line) {
        match orientation {
            LayoutOrientation::Vertical => {
                let left_start = Point2 {
                    x: breadth / 3.0,
                    y: 0.0,
                };
                let left_end = Point2 {
                    x: breadth / 3.0,
                    y: length,
                };
                let right_start = Point2 {
                    x: breadth / 3.0 + instrument_breadth,
                    y: 0.0,
                };
                let right_end = Point2 {
                    x: breadth / 3.0 + instrument_breadth,
                    y: length,
                };
                ((left_start, left_end), (right_start, right_end))
            }
            LayoutOrientation::Horizontal => {
                let left_start = Point2 {
                    x: 0.0,
                    y: breadth / 3.0 + instrument_breadth,
                };
                let left_end = Point2 {
                    x: length,
                    y: breadth / 3.0 + instrument_breadth,
                };
                let right_start = Point2 {
                    x: 0.0,
                    y: breadth / 3.0,
                };
                let right_end = Point2 {
                    x: length,
                    y: breadth / 3.0,
                };
                ((left_start, left_end), (right_start, right_end))
            }
        }
    }

    fn compute(
        space: Vector2<f32>,
        top_safe_area: f32,
        right_safe_area: f32,
        bottom_safe_area: f32,
        left_safe_area: f32,
    ) -> Option<Self> {
        let orientation = LayoutOrientation::from_space(space);
        let safe_area_padding = match orientation {
            LayoutOrientation::Horizontal => safe_area::horizontal(
                top_safe_area,
                right_safe_area,
                bottom_safe_area,
                left_safe_area,
            ),
            LayoutOrientation::Vertical => safe_area::vertical(
                top_safe_area,
                right_safe_area,
                bottom_safe_area,
                left_safe_area,
            ),
        };
        let length = orientation.length(space);
        let breadth = orientation.breadth(space);
        let safe_length = orientation.safe_length(space, safe_area_padding);
        let safe_breadth = orientation.safe_breadth(space, safe_area_padding);

        let instrument_breadth = safe_breadth / 3.0;

        let mut best_candidate: Option<LayoutCandidate> = None;

        for &num_keys_per_group in LAYOUT_PRIMES.iter() {
            if num_keys_per_group == 0 {
                continue;
            }
            for &num_groups in LAYOUT_PRIMES.iter() {
                if num_groups == 0 {
                    continue;
                }
                let total_keys = (num_keys_per_group as f32) * (num_groups as f32);
                let key_radius = (safe_length / total_keys) / 2.0;
                let key_band_breadth =
                    (instrument_breadth / 3.0).max(key_radius * 2.0 + MIN_BAND_PADDING);
                let key_bands_gap =
                    (safe_length - total_keys * key_radius * 2.0) / (total_keys + 1.0);
                let groups_gap = if num_groups > 1 {
                    (safe_length
                        - total_keys * key_radius * 2.0
                        - (num_groups as f32 - 1.0) * MIN_GAP)
                        / (num_groups as f32)
                } else {
                    safe_length - total_keys * key_radius * 2.0
                };

                let candidate = LayoutCandidate {
                    key_radius,
                    key_band_breadth,
                    key_bands_gap,
                    groups_gap,
                    num_keys_per_group,
                    num_groups,
                };

                if candidate.valid(instrument_breadth) {
                    match &best_candidate {
                        Some(best) => {
                            if candidate.key_radius > best.key_radius {
                                best_candidate = Some(candidate);
                            }
                        }
                        None => {
                            best_candidate = Some(candidate);
                        }
                    }
                }
            }
        }

        best_candidate.map(|final_candidate| {
            let (left_string_position, right_string_position) =
                Self::string_positions(orientation, breadth, length, instrument_breadth);

            let key_band_length = instrument_breadth * 2.0;

            Self {
                space,
                orientation,
                left_string_position,
                right_string_position,
                key_band_length,
                key_radius: final_candidate.key_radius,
                key_band_breadth: final_candidate.key_band_breadth,
                safe_area_padding,
                key_bands_gap: final_candidate.key_bands_gap,
                groups_gap: final_candidate.groups_gap,
                num_keys_per_group: NonZero::new(final_candidate.num_keys_per_group as u8).unwrap(),
                num_groups: NonZero::new(final_candidate.num_groups as u8).unwrap(),
            }
        })
    }

    fn fallback(
        space: Vector2<f32>,
        top_safe_area: f32,
        right_safe_area: f32,
        bottom_safe_area: f32,
        left_safe_area: f32,
    ) -> Self {
        let orientation = LayoutOrientation::from_space(space);
        let safe_area_padding = match orientation {
            LayoutOrientation::Horizontal => safe_area::horizontal(
                top_safe_area,
                right_safe_area,
                bottom_safe_area,
                left_safe_area,
            ),
            LayoutOrientation::Vertical => safe_area::vertical(
                top_safe_area,
                right_safe_area,
                bottom_safe_area,
                left_safe_area,
            ),
        };
        let length = orientation.length(space);
        let breadth = orientation.breadth(space);
        let safe_length = orientation.safe_length(space, safe_area_padding);
        let safe_breadth = orientation.safe_breadth(space, safe_area_padding);

        let instrument_breadth = safe_breadth / 3.0;

        let fallback_candidate = LayoutCandidate::fallback(safe_length, instrument_breadth);

        let (left_string_position, right_string_position) =
            Self::string_positions(orientation, breadth, length, instrument_breadth);

        let key_band_length = instrument_breadth * 2.0;

        Self {
            space,
            orientation,
            left_string_position,
            right_string_position,
            key_band_length,
            key_radius: fallback_candidate.key_radius,
            key_band_breadth: fallback_candidate.key_band_breadth,
            safe_area_padding,
            key_bands_gap: fallback_candidate.key_bands_gap,
            groups_gap: fallback_candidate.groups_gap,
            num_keys_per_group: NonZero::new(fallback_candidate.num_keys_per_group as u8).unwrap(),
            num_groups: NonZero::new(fallback_candidate.num_groups as u8).unwrap(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn has_primes() {
        println!("{LAYOUT_PRIMES:?}");
        assert_eq!(LAYOUT_PRIMES.len(), 54);
        assert_eq!(LAYOUT_PRIMES[0], 2);
        assert_eq!(LAYOUT_PRIMES[53], 251);
    }
}
