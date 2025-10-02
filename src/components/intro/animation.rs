//! Intro animation transform snapshot.
//!
//! This is a pure, tweenable snapshot of all primitive intro transform
//! variables (keys, bands, strings). It holds no timeline / phase / RAF
//! state and implements `CanTween` so external keyframe sequencing code
//! can interpolate between authored snapshots.
//!
//! MAYA DRY KISS:
//! - Keep only primitive fields needed for CSS var emission.
//! - No per-frame mutation logic lives here.
//! - No implicit phases, easing, or clocks: that belongs to the caller.
//!
//! Translation values remain 0 until real layout-based positioning is
//! integrated. Rotation is numeric (deg appended on emit). Band round
//! factor: 1 = circle, 0 = rectangle.
//!
//! Public surface:
//! - `IntroTransformSnapshot`
//! - `new(layout)` (identity / baseline)
//! - `emit_css_vars()`
//! - `impl CanTween` (lane-wise linear interpolation)

use keyframe::{num_traits::Float, CanTween};
use keyframe_derive::CanTween;
use shared::instrument::Layout as InstrumentLayout;

// Intro composition reference constants (SVG coordinate space 430x932)
const SUN_CX: f32 = 107.0;
const SUN_CY: f32 = 164.0;
const INITIAL_STRING_ROT_DEG: f32 = -17.1246;

/// Runtime storing only primitive arrays for CSS variable emission.
#[derive(Debug, Clone, Default)]
pub struct IntroTransformSnapshot {
    // Dimensions
    num_groups: u8,
    keys_per_group: u8,
    total: usize,

    // Key primitive transforms
    key_tx: Vec<f32>,
    key_ty: Vec<f32>,
    key_rot_deg: Vec<f32>, // numeric degrees; "deg" appended on emit
    key_sx: Vec<f32>,
    key_sy: Vec<f32>,

    // Band primitive transforms
    band_tx: Vec<f32>,
    band_ty: Vec<f32>,
    band_rot_deg: Vec<f32>,
    band_sx: Vec<f32>,
    band_sy: Vec<f32>,
    band_round: Vec<f32>, // 0=rect,1=circle

    // Strings (left; right reserved for extension)
    left_tx: f32,
    left_ty: f32,
    left_rot_deg: f32,
    left_sx: f32,
    left_sy: f32,

    // Cached capacity for CSS var string building
    css_capacity: usize,
}

impl IntroTransformSnapshot {
    /// Build a dummy snapshot aligned with the intro composition graphic.
    ///
    /// All keys and bands are translated so their centers coincide with the
    /// intro "sun" circle center (SUN_CX, SUN_CY). Bands are circular
    /// (round=1.0). Left string has the initial negative rotation. Scales
    /// remain 1.0 so when animation begins you can interpolate toward the
    /// real layout (which corresponds to zero translations).
    ///
    /// NOTE: This analytically reconstructs approximate centers using the
    /// same spacing model as the live keyboard layout. For higher fidelity
    /// per-key positions, replace the center calculation with authoritative
    /// layout-provided coordinates once exposed.
    pub fn dummy_with_layout(layout: InstrumentLayout) -> Self {
        let mut snap = Self::new(layout);
        let num_groups = snap.num_groups;
        let keys_per_group = snap.keys_per_group;

        // Shorthand (all f32 copies)
        let space = layout.space;
        let orientation = layout.orientation;
        let sa = layout.safe_area_padding;
        let key_band_length = layout.key_band_length;
        let key_band_breadth = layout.key_band_breadth;
        let groups_gap = layout.groups_gap;
        let key_bands_gap = layout.key_bands_gap;

        // Safe extents
        let safe_length = orientation.safe_length(space, sa);
        let safe_breadth = orientation.safe_breadth(space, sa);

        // Map safe-area to box sides (following keyboard.rs logic)
        let (safe_top, safe_right, safe_bottom, safe_left) = match orientation {
            shared::orientation::LayoutOrientation::Vertical => {
                // Vertical: indices are [top, left, bottom, right]
                (sa[0], sa[3], sa[2], sa[1])
            }
            shared::orientation::LayoutOrientation::Horizontal => {
                // Horizontal: indices are [left, top, right, bottom]
                (sa[1], sa[2], sa[3], sa[0])
            }
        };

        // Main-axis padding to center groups
        let groups_f = num_groups as f32;
        let required_length = groups_f * key_band_length + ((groups_f - 1.0).max(0.0) * groups_gap);
        let pad_main = ((safe_length - required_length) / 2.0).max(0.0);

        // Helper closures for center computation
        let mut center_for = |g: u8, k: u8| -> (f32, f32) {
            let g_f = g as f32;
            let k_f = k as f32;
            match orientation {
                shared::orientation::LayoutOrientation::Horizontal => {
                    // Groups laid out along X (length axis), keys stacked along Y (breadth axis)
                    let cx = safe_left
                        + pad_main
                        + g_f * (key_band_length + groups_gap)
                        + key_band_length * 0.5;
                    let cy = safe_top
                        + k_f * (key_band_breadth + key_bands_gap)
                        + key_band_breadth * 0.5;
                    (cx, cy)
                }
                shared::orientation::LayoutOrientation::Vertical => {
                    // Groups along Y, keys along X
                    let cy = safe_top
                        + pad_main
                        + g_f * (key_band_length + groups_gap)
                        + key_band_length * 0.5;
                    let cx = safe_left
                        + k_f * (key_band_breadth + key_bands_gap)
                        + key_band_breadth * 0.5;
                    (cx, cy)
                }
            }
        };

        for g in 0..num_groups {
            for k in 0..keys_per_group {
                let i = snap.idx(g, k);
                let (cx, cy) = center_for(g, k);
                // Translate so each center coincides with sun center
                snap.key_tx[i] = SUN_CX - cx;
                snap.key_ty[i] = SUN_CY - cy;
                snap.band_tx[i] = SUN_CX - cx;
                snap.band_ty[i] = SUN_CY - cy;

                snap.band_round[i] = 1.0; // circular at intro start
                                          // Rotations remain zero (no per-key/band rotation in intro start)
                                          // Scales already 1.0
            }
        }

        // Left string initial rotation; translation left at 0 (overlaid by picture)
        snap.left_rot_deg = INITIAL_STRING_ROT_DEG;

        snap
    }
    /// Create a new runtime with a layout snapshot.
    pub fn new(layout: InstrumentLayout) -> Self {
        let num_groups = layout.num_groups.get();
        let keys_per_group = layout.num_keys_per_group.get();
        let total = num_groups as usize * keys_per_group as usize;

        // Pre-size arrays (identity / zeros).
        let key_tx = vec![0.0; total];
        let key_ty = vec![0.0; total];
        let key_rot_deg = vec![0.0; total];
        let key_sx = vec![1.0; total];
        let key_sy = vec![1.0; total];

        let band_tx = vec![0.0; total];
        let band_ty = vec![0.0; total];
        let band_rot_deg = vec![0.0; total];
        let band_sx = vec![1.0; total];
        let band_sy = vec![1.0; total];
        let band_round = vec![0.0; total];

        // Strings baseline identity (will animate rotation only).
        let left_tx = 0.0;
        let left_ty = 0.0;
        let left_rot_deg = 0.0;
        let left_sx = 1.0;
        let left_sy = 1.0;

        // Rough capacity estimate:
        // Per key: 5 vars * ~24 chars ≈ 120
        // Per band: 6 vars * ~28 chars ≈ 168
        // Strings: ~5 * 28 ≈ 140
        // Margin: add 20%
        let css_capacity = ((total as f32) * (120.0 + 168.0) + 140.0) as usize * 12 / 10;

        let mut runtime = Self {
            num_groups,
            keys_per_group,
            total,
            key_tx,
            key_ty,
            key_rot_deg,
            key_sx,
            key_sy,
            band_tx,
            band_ty,
            band_rot_deg,
            band_sx,
            band_sy,
            band_round,
            left_tx,
            left_ty,
            left_rot_deg,
            left_sx,
            left_sy,
            css_capacity,
        };

        runtime.compute_baseline_key_band_transforms();
        runtime
    }

    /// Index helper.
    #[inline]
    fn idx(&self, g: u8, k: u8) -> usize {
        (g as usize) * (self.keys_per_group as usize) + (k as usize)
    }

    /// (Placeholder) baseline transform precomputation.
    ///
    /// Currently does nothing except reserve the possibility to integrate
    /// translation initialization once you inject real per-key positions.
    fn compute_baseline_key_band_transforms(&mut self) {
        // TODO: Fill in actual baseline translation offsets if you want keys/bands
        // to animate positionally relative to sun or collapsed point.
        // For now, all tx/ty remain 0 => final identity.
    }

    /// Reset all primitives to identity (utility for keyframe authors).
    pub fn reset_identity(&mut self) {
        for v in &mut self.key_tx {
            *v = 0.0;
        }
        for v in &mut self.key_ty {
            *v = 0.0;
        }
        for v in &mut self.key_rot_deg {
            *v = 0.0;
        }
        for v in &mut self.key_sx {
            *v = 1.0;
        }
        for v in &mut self.key_sy {
            *v = 1.0;
        }
        for v in &mut self.band_tx {
            *v = 0.0;
        }
        for v in &mut self.band_ty {
            *v = 0.0;
        }
        for v in &mut self.band_rot_deg {
            *v = 0.0;
        }
        for v in &mut self.band_sx {
            *v = 1.0;
        }
        for v in &mut self.band_sy {
            *v = 1.0;
        }
        for v in &mut self.band_round {
            *v = 0.0;
        }
        self.left_tx = 0.0;
        self.left_ty = 0.0;
        self.left_rot_deg = 0.0;
        self.left_sx = 1.0;
        self.left_sy = 1.0;
    }

    /// Build CSS variable assignment string from the snapshot.
    pub fn emit_css_vars(&self) -> String {
        let mut s = String::with_capacity(self.css_capacity);

        // Keys
        for g in 0..self.num_groups {
            for k in 0..self.keys_per_group {
                let i = self.idx(g, k);
                let fields = [
                    TransformField {
                        name: "tx",
                        kind: TransformKind::Num {
                            value: self.key_tx[i],
                            unit: Some("px"),
                        },
                    },
                    TransformField {
                        name: "ty",
                        kind: TransformKind::Num {
                            value: self.key_ty[i],
                            unit: Some("px"),
                        },
                    },
                    TransformField {
                        name: "rot",
                        kind: TransformKind::AngleDeg(self.key_rot_deg[i]),
                    },
                    TransformField {
                        name: "sx",
                        kind: TransformKind::Num {
                            value: self.key_sx[i],
                            unit: None,
                        },
                    },
                    TransformField {
                        name: "sy",
                        kind: TransformKind::Num {
                            value: self.key_sy[i],
                            unit: None,
                        },
                    },
                ];
                push_entity_vars(&mut s, "key", Some((g, k)), &fields);
            }
        }

        // Bands
        for g in 0..self.num_groups {
            for k in 0..self.keys_per_group {
                let i = self.idx(g, k);
                let fields = [
                    TransformField {
                        name: "tx",
                        kind: TransformKind::Num {
                            value: self.band_tx[i],
                            unit: Some("px"),
                        },
                    },
                    TransformField {
                        name: "ty",
                        kind: TransformKind::Num {
                            value: self.band_ty[i],
                            unit: Some("px"),
                        },
                    },
                    TransformField {
                        name: "rot",
                        kind: TransformKind::AngleDeg(self.band_rot_deg[i]),
                    },
                    TransformField {
                        name: "sx",
                        kind: TransformKind::Num {
                            value: self.band_sx[i],
                            unit: None,
                        },
                    },
                    TransformField {
                        name: "sy",
                        kind: TransformKind::Num {
                            value: self.band_sy[i],
                            unit: None,
                        },
                    },
                    TransformField {
                        name: "round",
                        kind: TransformKind::Num {
                            value: self.band_round[i],
                            unit: None,
                        },
                    },
                ];
                push_entity_vars(&mut s, "band", Some((g, k)), &fields);
            }
        }

        // Left string
        let left_fields = [
            TransformField {
                name: "tx",
                kind: TransformKind::Num {
                    value: self.left_tx,
                    unit: Some("px"),
                },
            },
            TransformField {
                name: "ty",
                kind: TransformKind::Num {
                    value: self.left_ty,
                    unit: Some("px"),
                },
            },
            TransformField {
                name: "rot",
                kind: TransformKind::AngleDeg(self.left_rot_deg),
            },
            TransformField {
                name: "sx",
                kind: TransformKind::Num {
                    value: self.left_sx,
                    unit: None,
                },
            },
            TransformField {
                name: "sy",
                kind: TransformKind::Num {
                    value: self.left_sy,
                    unit: None,
                },
            },
        ];
        push_entity_vars(&mut s, "left-string", None, &left_fields);

        s
    }

    // (removed unused circle_scale_x / circle_scale_y helper fns; snapshot stays minimal)
}

/// Push a u8 as decimal to string (avoids format! overhead in tight loops).
fn push_u8(s: &mut String, v: u8) {
    if v >= 100 {
        s.push(char::from(b'0' + (v / 100)));
        s.push(char::from(b'0' + ((v / 10) % 10)));
        s.push(char::from(b'0' + (v % 10)));
    } else if v >= 10 {
        s.push(char::from(b'0' + (v / 10)));
        s.push(char::from(b'0' + (v % 10)));
    } else {
        s.push(char::from(b'0' + v));
    }
}

/// Compact float formatting (3 decimal trimmed).
fn push_num(s: &mut String, v: f32) {
    let buf = itoa_or_short_f(v);
    s.push_str(&buf);
}

/// Format helper: For integral-ish values avoid trailing decimals, else 0.###
/// (Lightweight; not perfect but avoids heavy `format!`.)
fn itoa_or_short_f(v: f32) -> String {
    let iv = v as i32;
    if (v - iv as f32).abs() < 0.0005 {
        iv.to_string()
    } else {
        // Limit to 3 decimals
        let scaled = (v * 1000.0).round() as i32;
        let int_part = scaled / 1000;
        let frac = (scaled - int_part * 1000).abs();
        if frac == 0 {
            int_part.to_string()
        } else {
            // Trim trailing zeros
            let mut frac_str = format!("{:03}", frac);
            while frac_str.ends_with('0') {
                frac_str.pop();
            }
            format!("{int_part}.{frac_str}")
        }
    }
}

/// Generic transform field description.
struct TransformField<'a> {
    name: &'a str,
    kind: TransformKind,
}

/// Primitive transform value kinds we emit as CSS vars.
enum TransformKind {
    Num {
        value: f32,
        unit: Option<&'static str>,
    },
    AngleDeg(f32),
}

impl CanTween for IntroTransformSnapshot {
    fn ease(from: Self, to: Self, time: impl Float) -> Self {
        let t = time.to_f32().unwrap_or(0.0);
        #[inline]
        fn lerp(a: f32, b: f32, t: f32) -> f32 {
            a + (b - a) * t
        }

        // Clone 'from' to reuse allocations
        let mut out = from.clone();

        // All vector fields must have identical lengths between snapshots
        debug_assert_eq!(out.key_tx.len(), to.key_tx.len(), "key_tx length mismatch");
        let len = out.key_tx.len();

        // Helper macro to lerp same-length f32 vectors
        macro_rules! lerp_vec {
            ($field:ident) => {
                for i in 0..len {
                    out.$field[i] = lerp(from.$field[i], to.$field[i], t);
                }
            };
        }

        lerp_vec!(key_tx);
        lerp_vec!(key_ty);
        lerp_vec!(key_rot_deg);
        lerp_vec!(key_sx);
        lerp_vec!(key_sy);

        lerp_vec!(band_tx);
        lerp_vec!(band_ty);
        lerp_vec!(band_rot_deg);
        lerp_vec!(band_sx);
        lerp_vec!(band_sy);
        lerp_vec!(band_round);

        // Scalars
        out.left_tx = lerp(from.left_tx, to.left_tx, t);
        out.left_ty = lerp(from.left_ty, to.left_ty, t);
        out.left_rot_deg = lerp(from.left_rot_deg, to.left_rot_deg, t);
        out.left_sx = lerp(from.left_sx, to.left_sx, t);
        out.left_sy = lerp(from.left_sy, to.left_sy, t);

        out
    }
}

/// Emit a series of vars for a single entity (key/band/string).
fn push_entity_vars(
    out: &mut String,
    category: &str,
    gk: Option<(u8, u8)>,
    fields: &[TransformField],
) {
    for f in fields {
        out.push_str("--");
        out.push_str(category);
        if let Some((g, k)) = gk {
            out.push('-');
            push_u8(out, g);
            out.push('-');
            push_u8(out, k);
        }
        out.push('-');
        out.push_str(f.name);
        out.push(':');
        match f.kind {
            TransformKind::Num { value, unit } => {
                push_num(out, value);
                if let Some(u) = unit {
                    out.push_str(u);
                }
            }
            TransformKind::AngleDeg(v) => {
                push_num(out, v);
                out.push_str("deg");
            }
        }
        out.push(';');
    }
}
