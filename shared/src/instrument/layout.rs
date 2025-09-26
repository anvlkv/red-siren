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

#[derive(Debug, Clone, Copy)]
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
        let total_keys = 2.0;
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
            num_keys_per_group: 3,
            num_groups: 2,
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
    use crate::test_util::{
        DESKTOP_SCREEN_SIZES, MOBILE_SAFE_AREA_INSETS, MOBILE_SCREEN_SIZES,
        TABLET_SAFE_AREA_INSETS, TABLET_SCREEN_SIZES,
    };

    // Simple ASCII summarizer (intentionally approximate)
    fn ascii_layout(layout: &Layout) -> String {
        let groups = layout.num_groups.get() as usize;
        let keys_per_group = layout.num_keys_per_group.get() as usize;

        let shown = keys_per_group.min(12);
        let mut key_block = "o".repeat(shown);
        if keys_per_group > shown {
            key_block.push('+');
        }

        let mut gap_units = (layout.groups_gap / (layout.key_radius * 2.0))
            .round()
            .clamp(1.0, 8.0) as usize;
        if gap_units == 0 {
            gap_units = 1;
        }
        let gap = "-".repeat(gap_units);

        let body = (0..groups)
            .map(|_| format!("[{}]", key_block))
            .collect::<Vec<_>>()
            .join(&gap);

        let ori = match layout.orientation {
            LayoutOrientation::Horizontal => 'H',
            LayoutOrientation::Vertical => 'V',
        };

        format!(
            "{}x{} {} r={:.1} bw={:.1} g_gap≈{:.1} k_gap≈{:.1} groups={} keys/g={}: {}",
            layout.space.x as u32,
            layout.space.y as u32,
            ori,
            layout.key_radius,
            layout.key_band_breadth,
            layout.groups_gap,
            layout.key_bands_gap,
            layout.num_groups,
            layout.num_keys_per_group,
            body
        )
    }

    #[test]
    fn has_primes() {
        println!("PRIMES = {:?}", &LAYOUT_PRIMES[..]);
        println!("Count = {}", LAYOUT_PRIMES.len());
        assert_eq!(LAYOUT_PRIMES.len(), 54);
        assert_eq!(LAYOUT_PRIMES[0], 2);
        assert_eq!(LAYOUT_PRIMES[53], 251);
    }

    #[test]
    fn candidate_validity_conditions() {
        use super::{LayoutCandidate, MAX_KEY_RADIUS_TO_BREADTH_RATIO};

        let instrument_breadth = 300.0;

        let base = LayoutCandidate {
            key_radius: 32.0,
            key_band_breadth: 32.0 * 2.0 + MIN_BAND_PADDING + 4.0,
            key_bands_gap: MIN_GAP + 4.0,
            groups_gap: MIN_GAP + 4.0,
            num_keys_per_group: 5,
            num_groups: 3,
        };
        println!(
            "Base candidate (should be valid): r={} bw={} kgap={} ggap={}",
            base.key_radius, base.key_band_breadth, base.key_bands_gap, base.groups_gap
        );
        assert!(base.valid(instrument_breadth));

        let mut c = base;
        c.key_radius = MIN_KEY_RADIUS - 0.1;
        println!("Too small radius r={}", c.key_radius);
        assert!(!c.valid(instrument_breadth));

        let mut c = base;
        c.key_band_breadth = c.key_radius * 2.0 + MIN_BAND_PADDING - 0.5;
        println!(
            "Too small band breadth bw={} required>={}",
            c.key_band_breadth,
            c.key_radius * 2.0 + MIN_BAND_PADDING
        );
        assert!(!c.valid(instrument_breadth));

        let mut c = base;
        c.key_bands_gap = MIN_GAP - 0.5;
        println!("Too small key_bands_gap {}", c.key_bands_gap);
        assert!(!c.valid(instrument_breadth));

        let mut c = base;
        c.groups_gap = MIN_GAP - 0.5;
        println!("Too small groups_gap {}", c.groups_gap);
        assert!(!c.valid(instrument_breadth));

        let mut c = base;
        c.key_radius = instrument_breadth * (MAX_KEY_RADIUS_TO_BREADTH_RATIO + 0.01);
        println!(
            "Too large radius/breadth ratio r={} inst_b={}",
            c.key_radius, instrument_breadth
        );
        assert!(!c.valid(instrument_breadth));

        let mut c = base;
        c.num_keys_per_group = 0;
        println!("Zero keys per group");
        assert!(!c.valid(instrument_breadth));

        let mut c = base;
        c.num_groups = 0;
        println!("Zero groups");
        assert!(!c.valid(instrument_breadth));
    }

    #[test]
    fn string_positions_geometry() {
        let breadth = 600.0;
        let length = 1000.0;
        let instrument_breadth = breadth / 9.0; // arbitrary slice

        let (l_v, r_v) = Layout::string_positions(
            LayoutOrientation::Vertical,
            breadth,
            length,
            instrument_breadth,
        );
        println!("Vertical left={:?} right={:?}", l_v, r_v);
        assert_eq!(l_v.0.x, l_v.1.x);
        assert_eq!(r_v.0.x, r_v.1.x);
        assert!(r_v.0.x > l_v.0.x);
        assert_eq!(l_v.0.y, 0.0);
        assert_eq!(r_v.0.y, 0.0);
        assert_eq!(l_v.1.y, length);
        assert_eq!(r_v.1.y, length);

        let (l_h, r_h) = Layout::string_positions(
            LayoutOrientation::Horizontal,
            breadth,
            length,
            instrument_breadth,
        );
        println!("Horizontal left={:?} right={:?}", l_h, r_h);
        assert_eq!(l_h.0.y, l_h.1.y);
        assert_eq!(r_h.0.y, r_h.1.y);
        assert!(l_h.0.y > r_h.0.y);
        assert_eq!(l_h.0.x, 0.0);
        assert_eq!(r_h.0.x, 0.0);
        assert_eq!(l_h.1.x, length);
        assert_eq!(r_h.1.x, length);
    }

    #[test]
    fn common_screen_layouts_stats() {
        let mut total = 0usize;
        let mut compute_success = 0usize;
        let mut compute_fail = 0usize;

        fn attempt(space: (u32, u32), sa: (u32, u32, u32, u32)) -> (bool, Layout) {
            let v = mint::Vector2 {
                x: space.0 as f32,
                y: space.1 as f32,
            };
            let computed = Layout::compute(v, sa.0 as f32, sa.1 as f32, sa.2 as f32, sa.3 as f32);
            let layout = Layout::from_screen_estate_with_safe_area(
                v,
                sa.0 as f32,
                sa.1 as f32,
                sa.2 as f32,
                sa.3 as f32,
            );
            (computed.is_some(), layout)
        }

        println!("--- DESKTOP (NO SAFE AREA) ---");
        for size in DESKTOP_SCREEN_SIZES {
            total += 1;
            let (ok, layout) = attempt(size, (0, 0, 0, 0));
            println!(
                "size={:?} computed={} -> {}",
                size,
                ok,
                ascii_layout(&layout)
            );
            if ok {
                compute_success += 1
            } else {
                compute_fail += 1
            }
        }

        println!("--- DESKTOP (DEFAULT SAFE AREA) ---");
        let dsa = DEFAULT_SAFE_AREA as u32;
        for size in DESKTOP_SCREEN_SIZES {
            total += 1;
            let (ok, layout) = attempt(size, (dsa, dsa, dsa, dsa));
            println!(
                "size={:?} default_safe_area={} computed={} -> {}",
                size,
                dsa,
                ok,
                ascii_layout(&layout)
            );
            if ok {
                compute_success += 1
            } else {
                compute_fail += 1
            }
        }

        println!("--- MOBILE ---");
        for size in MOBILE_SCREEN_SIZES {
            for sa in MOBILE_SAFE_AREA_INSETS {
                total += 1;
                let (ok, layout) = attempt(size, sa);
                println!(
                    "size={:?} sa={:?} computed={} -> {}",
                    size,
                    sa,
                    ok,
                    ascii_layout(&layout)
                );
                if ok {
                    compute_success += 1
                } else {
                    compute_fail += 1
                }
            }
        }

        println!("--- TABLET ---");
        for size in TABLET_SCREEN_SIZES {
            for sa in TABLET_SAFE_AREA_INSETS {
                total += 1;
                let (ok, layout) = attempt(size, sa);
                println!(
                    "size={:?} sa={:?} computed={} -> {}",
                    size,
                    sa,
                    ok,
                    ascii_layout(&layout)
                );
                if ok {
                    compute_success += 1
                } else {
                    compute_fail += 1
                }
            }
        }

        println!(
            "SUMMARY: total={} success={} fail={} fallback_used={}",
            total, compute_success, compute_fail, compute_fail
        );

        assert_eq!(compute_success + compute_fail, total);
    }

    #[test]
    fn tiny_space_forces_fallback() {
        let space = mint::Vector2 { x: 40.0, y: 40.0 };
        let computed = Layout::compute(
            space,
            DEFAULT_SAFE_AREA,
            DEFAULT_SAFE_AREA,
            DEFAULT_SAFE_AREA,
            DEFAULT_SAFE_AREA,
        );
        println!("Tiny space computed={:?}", computed.is_some());
        assert!(computed.is_none());
        let layout = Layout::from_screen_estate(space);
        println!("Tiny fallback -> {}", ascii_layout(&layout));
        assert!(layout.key_radius < MIN_KEY_RADIUS);
    }

    #[test]
    fn huge_screen_layout() {
        let space = mint::Vector2 {
            x: 8000.0,
            y: 4000.0,
        };
        let layout = Layout::from_screen_estate(space);
        println!("Huge -> {}", ascii_layout(&layout));
        let orientation = LayoutOrientation::from_space(space);
        assert_eq!(layout.orientation, orientation);
        let safe_breadth = orientation.safe_breadth(space, layout.safe_area_padding);
        let instrument_breadth = safe_breadth / 3.0;
        assert!(layout.key_band_breadth >= layout.key_radius * 2.0);
        assert!(layout.key_radius < instrument_breadth * 2.0);
    }

    #[test]
    fn elongated_screens() {
        let horiz = mint::Vector2 {
            x: 10000.0,
            y: 300.0,
        };
        let vert = mint::Vector2 {
            x: 300.0,
            y: 10000.0,
        };
        let layout_h = Layout::from_screen_estate(horiz);
        let layout_v = Layout::from_screen_estate(vert);
        println!("Elongated H -> {}", ascii_layout(&layout_h));
        println!("Elongated V -> {}", ascii_layout(&layout_v));
        assert_eq!(layout_h.orientation, LayoutOrientation::Horizontal);
        assert_eq!(layout_v.orientation, LayoutOrientation::Vertical);
    }

    #[test]
    fn aggressive_safe_area() {
        let space = mint::Vector2 { x: 600.0, y: 900.0 };
        let top = 300.0;
        let bottom = 250.0;
        let layout = Layout::from_screen_estate_with_safe_area(space, top, 0.0, bottom, 0.0);
        println!(
            "Aggressive safe areas (t={}, b={}) -> {}",
            top,
            bottom,
            ascii_layout(&layout)
        );
        let safe_len = layout
            .orientation
            .safe_length(space, layout.safe_area_padding);
        assert!(safe_len > 0.0);
    }

    #[test]
    fn geometry_consistency_mid_sizes() {
        let cases = [
            mint::Vector2 {
                x: 1280.0,
                y: 720.0,
            },
            mint::Vector2 {
                x: 1440.0,
                y: 900.0,
            },
            mint::Vector2 {
                x: 768.0,
                y: 1024.0,
            },
        ];
        for space in cases {
            let layout = Layout::from_screen_estate(space);
            println!("Consistency -> {}", ascii_layout(&layout));
            assert!(layout.key_band_breadth >= 0.0);
            assert!(layout.key_bands_gap >= 0.0);
            assert!(layout.groups_gap >= 0.0);
        }
    }

    #[test]
    fn orientation_inference() {
        let h = mint::Vector2 { x: 500.0, y: 400.0 };
        let v = mint::Vector2 { x: 400.0, y: 500.0 };
        let sq = mint::Vector2 { x: 600.0, y: 600.0 };
        println!(
            "h -> {:?}  v -> {:?}  sq -> {:?}",
            LayoutOrientation::from_space(h),
            LayoutOrientation::from_space(v),
            LayoutOrientation::from_space(sq)
        );
        assert_eq!(
            LayoutOrientation::from_space(h),
            LayoutOrientation::Horizontal
        );
        assert_eq!(
            LayoutOrientation::from_space(v),
            LayoutOrientation::Vertical
        );
        assert_eq!(
            LayoutOrientation::from_space(sq),
            LayoutOrientation::Horizontal
        );
    }

    #[test]
    fn compute_failure_reason_analysis() {
        // Choose a representative desktop space where compute currently fails
        let space = mint::Vector2 {
            x: 1920.0,
            y: 1080.0,
        };
        let res = Layout::compute(
            space,
            DEFAULT_SAFE_AREA,
            DEFAULT_SAFE_AREA,
            DEFAULT_SAFE_AREA,
            DEFAULT_SAFE_AREA,
        );
        println!("Direct compute result present? {}", res.is_some());

        if res.is_some() {
            println!(
                "Compute succeeded unexpectedly for analysis target; skipping reason breakdown."
            );
            return;
        }

        // Reconstruct internal values mirroring compute()
        let orientation = LayoutOrientation::from_space(space);
        let safe_area_padding = match orientation {
            LayoutOrientation::Horizontal => safe_area::horizontal(
                DEFAULT_SAFE_AREA,
                DEFAULT_SAFE_AREA,
                DEFAULT_SAFE_AREA,
                DEFAULT_SAFE_AREA,
            ),
            LayoutOrientation::Vertical => safe_area::vertical(
                DEFAULT_SAFE_AREA,
                DEFAULT_SAFE_AREA,
                DEFAULT_SAFE_AREA,
                DEFAULT_SAFE_AREA,
            ),
        };
        let safe_length = orientation.safe_length(space, safe_area_padding);
        let safe_breadth = orientation.safe_breadth(space, safe_area_padding);
        let instrument_breadth = safe_breadth / 3.0;

        // Counters for invalidation reasons
        let mut too_small_radius = 0usize;
        let mut too_small_band_breadth = 0usize;
        let mut too_small_key_gap = 0usize;
        let mut too_small_group_gap = 0usize;
        let mut ratio_exceeded = 0usize;

        let mut total_candidates = 0usize;
        let mut first_invalid_examples: Vec<String> = Vec::new();

        for &num_keys_per_group in LAYOUT_PRIMES.iter() {
            for &num_groups in LAYOUT_PRIMES.iter() {
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

                total_candidates += 1;

                // Track invalid reasons (mirroring valid() predicate parts)
                let mut invalid_reasons = Vec::new();
                if key_radius < MIN_KEY_RADIUS {
                    too_small_radius += 1;
                    invalid_reasons.push("radius");
                }
                if key_band_breadth < (key_radius * 2.0 + MIN_BAND_PADDING) {
                    too_small_band_breadth += 1;
                    invalid_reasons.push("band_breadth");
                }
                if key_bands_gap < MIN_GAP {
                    too_small_key_gap += 1;
                    invalid_reasons.push("key_gap");
                }
                if groups_gap < MIN_GAP {
                    too_small_group_gap += 1;
                    invalid_reasons.push("group_gap");
                }
                if (key_radius / instrument_breadth) > MAX_KEY_RADIUS_TO_BREADTH_RATIO {
                    ratio_exceeded += 1;
                    invalid_reasons.push("radius_ratio");
                }

                if !invalid_reasons.is_empty() && first_invalid_examples.len() < 8 {
                    first_invalid_examples.push(format!(
                        "kpg={} groups={} r={:.2} bw={:.2} k_gap={:.2} g_gap={:.2} reasons={:?}",
                        num_keys_per_group,
                        num_groups,
                        key_radius,
                        key_band_breadth,
                        key_bands_gap,
                        groups_gap,
                        invalid_reasons
                    ));
                }
            }

            println!("Total candidates evaluated: {}", total_candidates);
            println!(
                "Invalid counts => radius:{} band_breadth:{} key_gap:{} group_gap:{} ratio:{}",
                too_small_radius,
                too_small_band_breadth,
                too_small_key_gap,
                too_small_group_gap,
                ratio_exceeded
            );
            println!("Sample invalid candidates:");
            for ex in &first_invalid_examples {
                println!("  {ex}");
            }

            // Sanity: at least one category should be non-zero if compute failed.
            assert!(
                too_small_radius > 0
                    || too_small_band_breadth > 0
                    || too_small_key_gap > 0
                    || too_small_group_gap > 0
                    || ratio_exceeded > 0
            );
        }
    }
}
