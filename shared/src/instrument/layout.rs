use std::num::NonZero;

use mint::{Point2, Vector2};
use serde::{Deserialize, Serialize};

use crate::{
    instrument::GroupChanel,
    orientation::LayoutOrientation,
    safe_area::{self, SafeArea, DEFAULT_SAFE_AREA},
    Line,
};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    /// Total screen estate available to layout the instrument
    pub space: Vector2<f32>,
    /// Whether `Horizontal` or `Vertical` layout is used
    pub orientation: LayoutOrientation,
    /// Start and end postions of **left** channel string
    pub left_string_position: Line,
    /// Start and end postions of **right** channel string
    pub right_string_position: Line,
    /// Radius of each key
    pub key_radius: f32,
    /// Length of the `track` of key's band
    pub key_band_length: f32,
    /// Breadth of the `track` of key's band
    pub key_band_breadth: f32,
    /// Minimum distance from edge of the screen to any interactive element
    pub safe_area_padding: SafeArea,
    /// Distance between `track`s (keys) inside a group (main axis)
    pub key_bands_gap: f32,
    /// Distance between groups (main axis)
    pub groups_gap: f32,
    /// Number of keys and bands in each group
    pub num_keys_per_group: NonZero<u8>,
    /// Number of groups
    pub num_groups: NonZero<u8>,
    /// Channel of the first group in the layout
    pub first_group_channel: super::GroupChanel,
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
            groups_gap: Default::default(),
            scale: Default::default(),
            // zero defaults
            space: Vector2 { x: 0.0, y: 0.0 },
            orientation: LayoutOrientation::Horizontal,
            left_string_position: (default_pt, default_pt),
            right_string_position: (default_pt, default_pt),
            num_keys_per_group: NonZero::new(1).unwrap(),
            num_groups: NonZero::new(1).unwrap(),
            first_group_channel: GroupChanel::Right,
        }
    }
}

impl Eq for Layout {}

const LAYOUT_PRIMES: const_primes::Primes<54> = const_primes::Primes::new();

const MIN_KEY_RADIUS: f32 = 16.0;
const MIN_BAND_PADDING: f32 = 8.0;
const MIN_GAP: f32 = 16.0;
const MIN_KEY_GAP_TO_GROUP_GAP_RATIO: f32 = 1.15;

const SOFT_RADIUS_RATIO: f32 = 0.40;
const ABSOLUTE_RADIUS_RATIO_MAX: f32 = 0.55;

// Gap model ratios (relative to diameter)
const KEY_GAP_RATIO_BASE: f32 = 0.25;
const KEY_GAP_MAX_RATIO: f32 = 0.90;
const GROUP_GAP_RATIO_MULTI: f32 = 1.20;
const GROUP_GAP_MAX_RATIO: f32 = 1.40;

// Packing target parameters
const BASE_PACK_TARGET: f32 = 0.42;
const PACK_SLOPE: f32 = 0.045;
const MIN_PACK: f32 = 0.35;
const MAX_PACK: f32 = 0.62;

// Leftover tolerance (fraction of safe length)
const LEFTOVER_TOLERANCE_FRAC: f32 = 0.04;

// Scoring weights
const W_R_SQRT: f32 = 0.60; // reduce radius dominance
const W_PACK: f32 = 0.50; // slight reduction
const W_STRUCT: f32 = 0.70; // boost structural richness (larger k)
const W_BALANCE: f32 = 0.20;
const W_LEFTOVER: f32 = 0.70;
const W_GAP_TENSION: f32 = 0.25;
const HIGH_K_BONUS: f32 = 0.08; // bonus for higher prime k

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct Candidate {
    g: u32,
    k: u32,
    r: f32,
    key_gap: f32,
    group_gap: f32,
    band_breadth: f32,
    packing_eff: f32,
    leftover: f32,
    score: f32,
    valid: bool,
    total_keys: u32,
    gap_tension: f32,
}

impl Candidate {
    fn layout(
        &self,
        space: Vector2<f32>,
        orientation: LayoutOrientation,
        safe_area_padding: SafeArea,
    ) -> Option<Layout> {
        if !self.valid {
            return None;
        }

        let first_group_channel = GroupChanel::from_keys_groups(self.k, self.g);

        Some(Layout {
            space,
            orientation,
            left_string_position: string_positions(orientation, space, true, self.band_breadth),
            right_string_position: string_positions(orientation, space, false, self.band_breadth),
            key_radius: self.r,
            key_band_length: self.band_breadth * 2.0,
            key_band_breadth: self.band_breadth,
            safe_area_padding,
            key_bands_gap: self.key_gap,
            groups_gap: self.group_gap,
            num_keys_per_group: NonZero::new(self.k as u8)?,
            num_groups: NonZero::new(self.g as u8)?,
            first_group_channel,
            scale: super::Scale::default(),
        })
    }
}

fn string_positions(
    orientation: LayoutOrientation,
    space: Vector2<f32>,
    left: bool,
    instrument_breadth: f32,
) -> Line {
    match orientation {
        LayoutOrientation::Vertical => {
            // Strings run along Y: two vertical lines that outline the instrument_breadth
            // and are centered in the available width (space.x).
            let center_x = space.x / 2.0;
            let half_b = instrument_breadth / 2.0;
            let left_x = (center_x - half_b).clamp(0.0, space.x);
            let right_x = (center_x + half_b).clamp(0.0, space.x);

            let x = if left { left_x } else { right_x };
            (
                Point2 { x, y: 0.0 },
                Point2 {
                    x,
                    y: orientation.length(space),
                },
            )
        }
        LayoutOrientation::Horizontal => {
            // Strings run along X: two horizontal lines that outline the instrument_breadth
            // and are centered in the available height (space.y).
            let center_y = space.y / 2.0;
            let half_b = instrument_breadth / 2.0;
            let top_y = (center_y - half_b).clamp(0.0, space.y);
            let bottom_y = (center_y + half_b).clamp(0.0, space.y);

            let y = if left { top_y } else { bottom_y };
            (
                Point2 { x: 0.0, y },
                Point2 {
                    x: orientation.length(space),
                    y,
                },
            )
        }
    }
}

fn adaptive_min_key_radius(safe_length: f32, instrument_breadth: f32) -> f32 {
    let scale_len = (safe_length / 600.0).clamp(0.85, 1.35);
    let scale_breadth = (instrument_breadth / 180.0).clamp(0.85, 1.30);
    let blended = 0.5 * (scale_len + scale_breadth);
    (MIN_KEY_RADIUS * blended).clamp(MIN_KEY_RADIUS * 0.70, MIN_KEY_RADIUS * 1.28)
}

fn enumerate(
    space: Vector2<f32>,
    orientation: LayoutOrientation,
    safe_area_padding: SafeArea,
) -> Vec<Candidate> {
    let safe_length = orientation.safe_length(space, safe_area_padding);
    let safe_breadth = orientation.safe_breadth(space, safe_area_padding);
    if safe_length <= 0.0 || safe_breadth <= 0.0 {
        return vec![];
    }
    let instrument_breadth = safe_breadth / 3.0;
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
            let tk_log = (total_keys as f32).ln_1p();
            let mut target_packing =
                (BASE_PACK_TARGET + PACK_SLOPE * tk_log).clamp(MIN_PACK, MAX_PACK);

            // initial radius guess
            let mut r = (safe_length * target_packing) / (2.0 * total_keys as f32);
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
            let mut group_gap = if g_have_gaps { MIN_GAP } else { 0.0 };
            for _ in 0..4 {
                let diameter = 2.0 * r;
                let key_gap_ratio = KEY_GAP_RATIO_BASE + (k as f32) / 60.0;
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
                group_gap = if g_have_gaps {
                    let desired = key_gap * GROUP_GAP_RATIO_MULTI;
                    let upper = (diameter * GROUP_GAP_MAX_RATIO).max(MIN_GAP);
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
                    && group_gap < key_gap * MIN_KEY_GAP_TO_GROUP_GAP_RATIO
                {
                    group_gap = key_gap * MIN_KEY_GAP_TO_GROUP_GAP_RATIO;
                }
                let intra_key_gap_count = g as f32 * (k.saturating_sub(1) as f32);
                let group_gap_count = (g.saturating_sub(1)) as f32;
                let used = diameter * total_keys as f32
                    + intra_key_gap_count * key_gap
                    + group_gap_count * group_gap;

                let packing_eff = (diameter * total_keys as f32) / safe_length;
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
            let intra_key_gap_count = g as f32 * (k.saturating_sub(1) as f32);
            let group_gap_count = (g.saturating_sub(1)) as f32;
            let used = diameter * total_keys as f32
                + intra_key_gap_count * key_gap
                + group_gap_count * group_gap;
            let leftover = (safe_length - used).max(0.0);
            let packing_eff = (diameter * total_keys as f32) / safe_length;

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
            if g_have_gaps && group_gap < MIN_GAP {
                valid = false;
            }
            if g_have_gaps
                && g_have_key_gaps
                && group_gap < key_gap * MIN_KEY_GAP_TO_GROUP_GAP_RATIO
            {
                valid = false;
            }
            if group_gap > diameter * GROUP_GAP_MAX_RATIO + 0.001 {
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
                let near = diameter * GROUP_GAP_MAX_RATIO * 0.95;
                if group_gap >= near {
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

            let struct_term = (total_keys as f32).ln_1p();
            let balance_term = 1.0 / (1.0 + (g as f32 - k as f32).abs());

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
                .map(|pos| ((pos as f32) / LAYOUT_PRIMES.len() as f32) * HIGH_K_BONUS)
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
                group_gap,
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
    space: Vector2<f32>,
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
    space: Vector2<f32>,
    orientation: LayoutOrientation,
    safe_area_padding: SafeArea,
) -> Layout {
    // Extremely small: single group of two keys basic layout
    let _safe_length = orientation.safe_length(space, safe_area_padding).max(1.0);
    let safe_breadth = orientation.safe_breadth(space, safe_area_padding).max(1.0);
    let instrument_breadth = safe_breadth / 3.0;
    let r = {
        let raw = instrument_breadth * 0.25;
        // Avoid panics from reversed clamp bounds when raw < 6 or MIN_KEY_RADIUS < 6 (future tweaks)
        let lower = 6.0_f32.min(MIN_KEY_RADIUS);
        let upper = 6.0_f32.max(MIN_KEY_RADIUS);
        raw.max(lower).min(upper)
    };
    let key_gap = MIN_GAP;
    let group_gap = 0.0;
    let band_breadth = (2.0 * r + MIN_BAND_PADDING).max(instrument_breadth / 4.0);

    Layout {
        space,
        orientation,
        left_string_position: string_positions(orientation, space, true, band_breadth),
        right_string_position: string_positions(orientation, space, false, band_breadth),
        key_radius: r,
        key_band_length: band_breadth * 2.0,
        key_band_breadth: band_breadth,
        safe_area_padding,
        key_bands_gap: key_gap,
        groups_gap: group_gap,
        num_keys_per_group: NonZero::new(2).unwrap(),
        num_groups: NonZero::new(1).unwrap(),
        first_group_channel: GroupChanel::from_keys_groups(2, 1),
        scale: super::Scale::default(),
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

        let best = pick_best(space, orientation, safe_area_padding)?;
        best.layout(space, orientation, safe_area_padding)
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
        fallback(space, orientation, safe_area_padding)
    }
}

#[cfg(any(test, feature = "test"))]
pub fn layout_test_cases() -> impl Iterator<Item = Layout> {
    crate::test_util::test_cases().map(|(space, safe_area)| {
        let v = Vector2 {
            x: space.0 as f32,
            y: space.1 as f32,
        };
        Layout::from_screen_estate_with_safe_area(
            v,
            safe_area.0,
            safe_area.1,
            safe_area.2,
            safe_area.3,
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ascii_summary(layout: &Layout) -> String {
        let g = layout.num_groups.get();
        let k = layout.num_keys_per_group.get();
        let shown = k.min(13);
        let mut block = "o".repeat(shown as usize);
        if k > shown {
            block.push('+');
        }
        let mut groups = Vec::new();
        for _ in 0..g {
            groups.push(format!("[{}]", block));
        }
        let gap_units = if layout.key_bands_gap <= 0.0 {
            1
        } else {
            ((layout.key_bands_gap / (layout.key_radius * 2.0))
                .round()
                .clamp(1.0, 8.0)) as usize
        };
        let gap = "-".repeat(gap_units);
        format!(
            "{}x{} {} r={:.1} bw={:.1} g_gap≈{:.1} k_gap≈{:.1} groups={} keys/g={} : {}",
            layout.space.x as u32,
            layout.space.y as u32,
            match layout.orientation {
                LayoutOrientation::Horizontal => 'H',
                LayoutOrientation::Vertical => 'V',
            },
            layout.key_radius,
            layout.key_band_breadth,
            layout.groups_gap,
            layout.key_bands_gap,
            g,
            k,
            groups.join(&gap)
        )
    }

    #[test]
    fn scoring_diagnostics_large_desktop() {
        let space = Vector2 {
            x: 2560.0,
            y: 1440.0,
        };
        let ori = LayoutOrientation::from_space(space);
        let sap = safe_area::horizontal(
            DEFAULT_SAFE_AREA,
            DEFAULT_SAFE_AREA,
            DEFAULT_SAFE_AREA,
            DEFAULT_SAFE_AREA,
        );
        let cands = super::enumerate(space, ori, sap);

        let mut valids: Vec<_> = cands.into_iter().filter(|c| c.valid).collect();
        assert!(!valids.is_empty(), "Expected at least one valid candidate");
        valids.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let max_score = valids.first().unwrap().score;

        // Print top 12 and also any with k in {7,11,13}
        println!("--- TOP 12 ---");
        for c in valids.iter().take(12) {
            println!(
                "g={} k={} r={:.1} key_gap={:.1} group_gap={:.1} total={} pack={:.3} left={:.1} score={:.3}",
                c.g, c.k, c.r, c.key_gap, c.group_gap, c.total_keys, c.packing_eff, c.leftover, c.score
            );
        }
        println!("--- LARGE k presence (7,11,13) ---");
        let mut large_presence = false;
        for c in valids.iter().filter(|c| [7, 11, 13].contains(&c.k)) {
            println!(
                "LARGE k candidate -> g={} k={} r={:.1} score={:.3} ({}% of max)",
                c.g,
                c.k,
                c.r,
                c.score,
                (c.score / max_score * 100.0)
            );
            if c.score >= max_score * 0.60 {
                large_presence = true;
            }
        }
        assert!(
            large_presence,
            "Expected at least one k in {{7,11,13}} within 60% of top score"
        );
    }

    #[test]
    fn layouts_across_common_sets() {
        let mut any_large_prime = false;
        for layout in layout_test_cases() {
            println!("{}", ascii_summary(&layout));
            if [7, 11, 13].contains(&layout.num_keys_per_group.get()) {
                any_large_prime = true;
            }
            // Core validity invariants
            assert!(layout.key_radius >= MIN_KEY_RADIUS * 0.85);
            if layout.num_keys_per_group.get() > 1 {
                assert!(layout.key_bands_gap >= MIN_GAP);
            } else {
                assert!(layout.key_bands_gap == 0.0);
            }
            if layout.num_groups.get() > 1 {
                assert!(layout.groups_gap >= MIN_GAP);
                if layout.num_keys_per_group.get() > 1 {
                    assert!(
                        layout.groups_gap >= layout.key_bands_gap * MIN_KEY_GAP_TO_GROUP_GAP_RATIO
                    );
                }
            } else {
                assert!(layout.groups_gap == 0.0);
            }
        }
        assert!(
            any_large_prime,
            "Expected at least one layout selecting keys-per-group in {{7,11,13}}"
        );
    }
}
