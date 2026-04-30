use std::num::NonZero;

use mint::{Point2, Vector2};
use serde::{Deserialize, Serialize};

use crate::{
    instrument::BandChannel,
    orientation::LayoutOrientation,
    safe_area::{SafeArea, DEFAULT_SAFE_AREA},
    Line, NodeKeyRegistry,
};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    /// Total screen estate available to layout the instrument
    pub space: Vector2<f64>,
    /// Whether `Horizontal` or `Vertical` layout is used
    pub orientation: LayoutOrientation,
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
    /// Minimum distance from edge of the screen to any interactive element
    pub safe_area_padding: SafeArea,
    /// Distance between `track`s (keys) inside a band (main axis)
    pub key_bands_gap: f64,
    /// Distance between bands (main axis)
    pub bands_gap: f64,
    /// Number of keys and bands in each band
    pub num_keys_per_band: NonZero<u8>,
    /// Number of bands
    pub num_bands: NonZero<u8>,
    /// Channel of the first band in the layout
    pub first_band_channel: super::BandChannel,
    /// Whether dark or bright scale is used
    ///
    /// `Scale::Yo` by default
    ///
    /// change to `Scale::In` before producing `instrument::Config` if necessary
    pub scale: super::Scale,
}

impl Default for Layout {
    fn default() -> Self {
        let default_pt = Point2 { x: 0.0, y: 0.0 };
        Self {
            key_radius: Default::default(),
            key_band_length: Default::default(),
            key_band_breadth: Default::default(),
            safe_area_padding: Default::default(),
            key_bands_gap: Default::default(),
            bands_gap: Default::default(),
            scale: Default::default(),
            // zero defaults
            instrument_breadth: 0.0,
            space: Vector2 { x: 0.0, y: 0.0 },
            orientation: LayoutOrientation::Horizontal,
            left_string_position: (default_pt, default_pt),
            right_string_position: (default_pt, default_pt),
            num_keys_per_band: NonZero::new(2).unwrap(),
            num_bands: NonZero::new(2).unwrap(),
            first_band_channel: BandChannel::Right,
        }
    }
}

impl Eq for Layout {}

/// Helper / utility methods shared by intro animation & instrument layout
impl Layout {
    /// Number of bands as f64
    #[inline]
    fn bands_f(&self) -> f64 {
        self.num_bands.get() as f64
    }

    /// Main–axis (bands axis) padding used to center the grouped bands.
    #[inline]
    pub fn key_pad_main(&self) -> f64 {
        let safe_len = self
            .orientation
            .safe_length(self.space, self.safe_area_padding);
        let bands = self.bands_f();
        if bands <= 0.0 {
            return 0.0;
        }
        let required = bands * self.key_band_length + (bands - 1.0) * self.bands_gap;
        ((safe_len - required) / 2.0).max(0.0)
    }

    pub fn registry(&self) -> NodeKeyRegistry {
        NodeKeyRegistry::new(self.num_bands.get(), self.num_keys_per_band.get())
    }
}

pub const LAYOUT_PRIMES: const_primes::Primes<20> = const_primes::Primes::new();

const MIN_KEY_RADIUS: f64 = 16.0;
const MIN_BAND_PADDING: f64 = 8.0;
const MIN_GAP: f64 = 16.0;
const MIN_KEY_GAP_TO_BAND_GAP_RATIO: f64 = 1.15;

const SOFT_RADIUS_RATIO: f64 = 0.40;
const ABSOLUTE_RADIUS_RATIO_MAX: f64 = 0.55;

// Gap model ratios (relative to diameter)
const KEY_GAP_RATIO_BASE: f64 = 0.25;
const KEY_GAP_MAX_RATIO: f64 = 0.90;
const BAND_GAP_RATIO_MULTI: f64 = 1.20;
const BAND_GAP_MAX_RATIO: f64 = 1.40;
const STRING_TO_BAND_MIN_GAP_RATIO: f64 = 0.13;

// Packing target parameters
const BASE_PACK_TARGET: f64 = 0.42;
const PACK_SLOPE: f64 = 0.045;
const MIN_PACK: f64 = 0.35;
const MAX_PACK: f64 = 0.62;

// Leftover tolerance (fraction of safe length)
const LEFTOVER_TOLERANCE_FRAC: f64 = 0.04;

// Scoring weights
const W_R_SQRT: f64 = 0.60; // reduce radius dominance
const W_PACK: f64 = 0.50; // slight reduction
const W_STRUCT: f64 = 0.70; // boost structural richness (larger k)
const W_BALANCE: f64 = 0.20;
const W_LEFTOVER: f64 = 0.70;
const W_GAP_TENSION: f64 = 0.25;
const HIGH_K_BONUS: f64 = 0.08; // bonus for higher prime k

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
struct Candidate {
    g: u32,
    k: u32,
    r: f64,
    key_gap: f64,
    band_gap: f64,
    band_breadth: f64,
    packing_eff: f64,
    leftover: f64,
    score: f64,
    valid: bool,
    total_keys: u32,
    gap_tension: f64,
}

impl Candidate {
    fn layout(
        &self,
        space: Vector2<f64>,
        orientation: LayoutOrientation,
        safe_area_padding: SafeArea,
    ) -> Option<Layout> {
        if !self.valid {
            return None;
        }

        let first_band_channel = BandChannel::Right;
        let safe_breadth = orientation.safe_breadth(space, safe_area_padding).max(1.0);
        let instrument_breadth = self.band_breadth * (1.0 + 2.0 * STRING_TO_BAND_MIN_GAP_RATIO);
        let band_length = (safe_breadth - instrument_breadth) / 2.0;

        Some(Layout {
            space,
            orientation,
            left_string_position: string_positions(
                orientation,
                space,
                safe_area_padding,
                true,
                instrument_breadth,
            ),
            right_string_position: string_positions(
                orientation,
                space,
                safe_area_padding,
                false,
                instrument_breadth,
            ),
            instrument_breadth,
            key_radius: self.r,
            key_band_length: band_length,
            key_band_breadth: self.band_breadth,
            safe_area_padding,
            key_bands_gap: self.key_gap,
            bands_gap: self.band_gap,
            num_keys_per_band: NonZero::new(self.k as u8)?,
            num_bands: NonZero::new(self.g as u8)?,
            first_band_channel,
            scale: super::Scale::default(),
        })
    }
}

fn string_positions(
    orientation: LayoutOrientation,
    space: Vector2<f64>,
    safe_area: SafeArea,
    left: bool,
    instrument_breadth: f64,
) -> Line {
    match orientation {
        LayoutOrientation::Vertical => {
            // Strings run along Y. Center along auxiliary axis (X) within safe area.
            let safe_x_start = safe_area.left;
            let safe_x_end = (space.x - safe_area.right).max(safe_x_start);
            let center_x = safe_x_start + (safe_x_end - safe_x_start) / 2.0;
            let half_b = instrument_breadth / 2.0;
            let left_x = (center_x - half_b).clamp(safe_x_start, safe_x_end);
            let right_x = (center_x + half_b).clamp(safe_x_start, safe_x_end);

            let x = if left { left_x } else { right_x };
            (
                Point2 { x, y: 0.0 },
                Point2 {
                    x,
                    // Do not apply safe area along main axis
                    y: orientation.length(space),
                },
            )
        }
        LayoutOrientation::Horizontal => {
            // Strings run along X. Center along auxiliary axis (Y) within safe area.
            let safe_y_start = safe_area.top;
            let safe_y_end = (space.y - safe_area.bottom).max(safe_y_start);
            let center_y = safe_y_start + (safe_y_end - safe_y_start) / 2.0;
            let half_b = instrument_breadth / 2.0;
            let top_y = (center_y - half_b).clamp(safe_y_start, safe_y_end);
            let bottom_y = (center_y + half_b).clamp(safe_y_start, safe_y_end);

            let y = if left { top_y } else { bottom_y };
            (
                Point2 { x: 0.0, y },
                Point2 {
                    // Do not apply safe area along main axis
                    x: orientation.length(space),
                    y,
                },
            )
        }
    }
}

fn adaptive_min_key_radius(safe_length: f64, instrument_breadth: f64) -> f64 {
    let scale_len = (safe_length / 600.0).clamp(0.85, 1.35);
    let scale_breadth = (instrument_breadth / 180.0).clamp(0.85, 1.30);
    let blended = 0.5 * (scale_len + scale_breadth);
    (MIN_KEY_RADIUS * blended).clamp(MIN_KEY_RADIUS * 0.70, MIN_KEY_RADIUS * 1.28)
}

fn enumerate(
    space: Vector2<f64>,
    orientation: LayoutOrientation,
    safe_area_padding: SafeArea,
) -> Vec<Candidate> {
    let safe_breadth = orientation.safe_breadth(space, safe_area_padding);
    let instrument_breadth = safe_breadth / 3.0;
    let safe_length = orientation.safe_length(space, safe_area_padding) - instrument_breadth;
    if safe_length <= 0.0 || safe_breadth <= 0.0 {
        return vec![];
    }
    let r_cap = instrument_breadth * SOFT_RADIUS_RATIO;
    let abs_r_cap = instrument_breadth * ABSOLUTE_RADIUS_RATIO_MAX;
    let adaptive_min = adaptive_min_key_radius(safe_length, instrument_breadth);

    // Choose primes subsets
    let prim_g: Vec<u32> = LAYOUT_PRIMES.iter().copied().collect();

    let prim_k: Vec<u32> = LAYOUT_PRIMES.iter().copied().collect();

    let mut out = Vec::new();

    for &g in &prim_g {
        for &k in prim_k.iter().filter(|k| **k != g) {
            let total_keys = g * k;
            // target packing increases slowly with total keys (diminishing returns)
            let tk_log = (total_keys as f64).ln_1p();
            let mut target_packing =
                (BASE_PACK_TARGET + PACK_SLOPE * tk_log).clamp(MIN_PACK, MAX_PACK);

            // initial radius guess
            let mut r = (safe_length * target_packing) / (2.0 * total_keys as f64);
            // Enforce adaptive floor before applying cap to avoid tiny diameter that inverts later gap clamps
            r = r.max(adaptive_min);
            r = r.min(r_cap);

            let g_have_gaps = g > 1;
            let g_have_key_gaps = k > 1;
            if !g_have_key_gaps && !g_have_gaps {
                continue; // trivial 1x1 not interesting
            }

            // iterative solve to incorporate gap dependency
            let mut key_gap = MIN_GAP;
            let mut band_gap = if g_have_gaps { MIN_GAP } else { 0.0 };
            for _ in 0..4 {
                let diameter = 2.0 * r;
                let key_gap_ratio = KEY_GAP_RATIO_BASE + (k as f64) / 60.0;
                key_gap = if g_have_key_gaps {
                    let desired = diameter * key_gap_ratio;
                    let upper = (diameter * KEY_GAP_MAX_RATIO).max(MIN_GAP);
                    if desired < MIN_GAP {
                        MIN_GAP
                    } else if desired > upper {
                        upper
                    } else {
                        desired
                    }
                } else {
                    0.0
                };
                band_gap = if g_have_gaps {
                    let desired = key_gap * BAND_GAP_RATIO_MULTI;
                    let upper = (diameter * BAND_GAP_MAX_RATIO).max(MIN_GAP);
                    if desired < MIN_GAP {
                        MIN_GAP
                    } else if desired > upper {
                        upper
                    } else {
                        desired
                    }
                } else {
                    0.0
                };
                // enforce ratio rule if both conceptual
                if g_have_gaps
                    && g_have_key_gaps
                    && band_gap < key_gap * MIN_KEY_GAP_TO_BAND_GAP_RATIO
                {
                    band_gap = key_gap * MIN_KEY_GAP_TO_BAND_GAP_RATIO;
                }
                let intra_key_gap_count = g as f64 * (k.saturating_sub(1) as f64);
                let band_gap_count = (g.saturating_sub(1)) as f64;
                let used = diameter * total_keys as f64
                    + intra_key_gap_count * key_gap
                    + band_gap_count * band_gap;

                let packing_eff = (diameter * total_keys as f64) / safe_length;
                // adjust target_packing slightly upward if actual packing too low but leftover big
                if packing_eff + 0.03 < target_packing {
                    target_packing = (target_packing * 0.98).max(MIN_PACK);
                }

                if used > safe_length * target_packing {
                    let scale = (safe_length * target_packing) / used;
                    r *= scale;
                } else {
                    // we could try to expand a little (but stay <= r_cap)
                    let headroom = r_cap - r;
                    if headroom > 1.0 {
                        r += headroom * 0.15;
                    }
                }
                r = r.min(r_cap);
            }

            // final geometry
            let diameter = 2.0 * r;
            let intra_key_gap_count = g as f64 * (k.saturating_sub(1) as f64);
            let band_gap_count = (g.saturating_sub(1)) as f64;
            let used = diameter * total_keys as f64
                + intra_key_gap_count * key_gap
                + band_gap_count * band_gap;
            let leftover = (safe_length - used).max(0.0);
            let packing_eff = (diameter * total_keys as f64) / safe_length;

            // validity
            let mut valid = true;
            if r < adaptive_min.max(MIN_KEY_RADIUS * 0.85) {
                valid = false;
            }
            if r > abs_r_cap {
                valid = false;
            }
            if g_have_key_gaps && key_gap < MIN_GAP {
                valid = false;
            }
            if g_have_gaps && band_gap < MIN_GAP {
                valid = false;
            }
            if g_have_gaps && g_have_key_gaps && band_gap < key_gap * MIN_KEY_GAP_TO_BAND_GAP_RATIO
            {
                valid = false;
            }
            if band_gap > diameter * BAND_GAP_MAX_RATIO + 0.001 {
                valid = false;
            }
            if key_gap > diameter * KEY_GAP_MAX_RATIO + 0.001 {
                valid = false;
            }
            if (r / instrument_breadth) > ABSOLUTE_RADIUS_RATIO_MAX {
                valid = false;
            }
            let band_breadth = (diameter + MIN_BAND_PADDING).max(instrument_breadth / 3.2);
            if band_breadth < diameter + MIN_BAND_PADDING {
                valid = false;
            }

            // gap tension if near max
            let mut gap_tension = 0.0;
            if g_have_key_gaps {
                let near = diameter * KEY_GAP_MAX_RATIO * 0.95;
                if key_gap >= near {
                    gap_tension += 0.5;
                }
            }
            if g_have_gaps {
                let near = diameter * BAND_GAP_MAX_RATIO * 0.95;
                if band_gap >= near {
                    gap_tension += 0.7;
                }
            }

            // scoring components
            let r_norm = if r_cap > 0.0 {
                (r / r_cap).min(1.0)
            } else {
                0.0
            };
            let r_term = r_norm.powf(0.6);

            let struct_term = (total_keys as f64).ln_1p();
            let balance_term = 1.0 / (1.0 + (g as f64 - k as f64).abs());

            let leftover_fraction = if safe_length > 0.0 {
                leftover / safe_length
            } else {
                1.0
            };

            let leftover_penalty = if leftover_fraction > LEFTOVER_TOLERANCE_FRAC {
                let adj = leftover_fraction - LEFTOVER_TOLERANCE_FRAC;
                adj * adj
            } else {
                0.0
            };

            let high_k_bonus = LAYOUT_PRIMES
                .iter()
                .position(|p| p == &k)
                .map(|pos| ((pos as f64) / LAYOUT_PRIMES.len() as f64) * HIGH_K_BONUS)
                .unwrap_or(0.0);

            let score = W_R_SQRT * r_term
                + W_PACK * packing_eff
                + W_STRUCT * struct_term
                + W_BALANCE * balance_term
                - W_LEFTOVER * leftover_penalty
                - W_GAP_TENSION * gap_tension
                + high_k_bonus;

            out.push(Candidate {
                g,
                k,
                r,
                key_gap,
                band_gap,
                band_breadth,
                packing_eff,
                leftover,
                score: if valid { score } else { -1_000_000.0 },
                valid,
                total_keys,
                gap_tension,
            });
        }
    }

    out
}

fn pick_best(
    space: Vector2<f64>,
    orientation: LayoutOrientation,
    safe_area_padding: SafeArea,
) -> Option<Candidate> {
    let mut cands = enumerate(space, orientation, safe_area_padding);
    if cands.is_empty() {
        return None;
    }

    // Keep only valid
    cands.retain(|c| c.valid);
    if cands.is_empty() {
        return None;
    }

    // Tie-break order:
    // 1. higher score
    // 2. higher total_keys
    // 3. higher packing_eff
    // 4. larger min(g,k)
    // 5. smaller leftover
    cands.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.total_keys.cmp(&a.total_keys))
            .then_with(|| {
                b.packing_eff
                    .partial_cmp(&a.packing_eff)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| {
                let a_min = a.g.min(a.k);
                let b_min = b.g.min(b.k);
                b_min.cmp(&a_min)
            })
            .then_with(|| {
                a.leftover
                    .partial_cmp(&b.leftover)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });

    cands.into_iter().next()
}

fn fallback(
    space: Vector2<f64>,
    orientation: LayoutOrientation,
    safe_area_padding: SafeArea,
) -> Layout {
    // Extremely small: single group of two keys basic layout
    let safe_breadth = orientation.safe_breadth(space, safe_area_padding).max(1.0);
    let instrument_breadth = safe_breadth / 3.0;
    let r = {
        let raw = instrument_breadth * 0.25;
        // Avoid panics from reversed clamp bounds when raw < 6 or MIN_KEY_RADIUS < 6 (future tweaks)
        let lower = 6.0_f64.min(MIN_KEY_RADIUS);
        let upper = 6.0_f64.max(MIN_KEY_RADIUS);
        raw.max(lower).min(upper)
    };
    let key_gap = MIN_GAP;
    let band_gap = 0.0;
    let band_breadth = (2.0 * r + MIN_BAND_PADDING).max(instrument_breadth / 4.0);
    let band_length =
        (orientation.safe_breadth(space, safe_area_padding) / 2.0) - instrument_breadth;

    let instrument_breadth = band_breadth * (1.0 + 2.0 * STRING_TO_BAND_MIN_GAP_RATIO);

    Layout {
        space,
        orientation,
        instrument_breadth,
        left_string_position: string_positions(
            orientation,
            space,
            safe_area_padding,
            true,
            instrument_breadth,
        ),
        right_string_position: string_positions(
            orientation,
            space,
            safe_area_padding,
            false,
            instrument_breadth,
        ),
        key_radius: r,
        key_band_length: band_length,
        key_band_breadth: band_breadth,
        safe_area_padding,
        key_bands_gap: key_gap,
        bands_gap: band_gap,
        num_keys_per_band: NonZero::new(2).unwrap(),
        num_bands: NonZero::new(2).unwrap(),
        first_band_channel: BandChannel::Right,
        scale: super::Scale::default(),
    }
}

impl Layout {
    pub fn from_screen_estate(space: Vector2<f64>) -> Self {
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
        top_safe_area: f64,
        right_safe_area: f64,
        bottom_safe_area: f64,
        left_safe_area: f64,
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
        space: Vector2<f64>,
        top_safe_area: f64,
        right_safe_area: f64,
        bottom_safe_area: f64,
        left_safe_area: f64,
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

    fn compute(
        space: Vector2<f64>,
        top_safe_area: f64,
        right_safe_area: f64,
        bottom_safe_area: f64,
        left_safe_area: f64,
    ) -> Option<Self> {
        let orientation = LayoutOrientation::from_space(space);
        let safe_area_padding = SafeArea {
            top: top_safe_area,
            right: right_safe_area,
            bottom: bottom_safe_area,
            left: left_safe_area,
        };

        let best = pick_best(space, orientation, safe_area_padding)?;
        best.layout(space, orientation, safe_area_padding)
    }

    fn fallback(
        space: Vector2<f64>,
        top_safe_area: f64,
        right_safe_area: f64,
        bottom_safe_area: f64,
        left_safe_area: f64,
    ) -> Self {
        let orientation = LayoutOrientation::from_space(space);
        let safe_area_padding = SafeArea {
            top: top_safe_area,
            right: right_safe_area,
            bottom: bottom_safe_area,
            left: left_safe_area,
        };
        fallback(space, orientation, safe_area_padding)
    }
}

#[cfg(any(test, feature = "test"))]
pub fn layout_test_cases() -> impl Iterator<Item = Layout> {
    crate::test_util::test_cases()
        .enumerate()
        .map(|(i, (space, safe_area))| {
            let v = Vector2 {
                x: space.0 as f64,
                y: space.1 as f64,
            };
            Layout {
                scale: if i.is_multiple_of(2) {
                    super::Scale::In
                } else {
                    super::Scale::Yo
                },
                ..Layout::from_screen_estate_with_safe_area(
                    v,
                    safe_area.0,
                    safe_area.1,
                    safe_area.2,
                    safe_area.3,
                )
            }
        })
}

#[cfg(test)]
mod tests {
    use insta::assert_json_snapshot;

    use super::*;

    #[test]
    fn scoring_diagnostics_large_desktop() {
        let space = Vector2 {
            x: 2560.0,
            y: 1440.0,
        };
        let ori = LayoutOrientation::from_space(space);
        let sap = SafeArea::default();
        let cands = super::enumerate(space, ori, sap);

        assert_json_snapshot!(cands)
    }

    #[test]
    fn layouts_across_common_sets() {
        for layout in layout_test_cases() {
            assert_json_snapshot!(
                format!("instrument_layout_{}x{}", layout.space.x, layout.space.y),
                layout
            )
        }
    }

    #[test]
    fn primes() {
        assert_json_snapshot!(LAYOUT_PRIMES.iter().copied().collect::<Vec<u32>>());
    }
}
