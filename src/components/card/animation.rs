/*!
Card animation constants & helpers (MAYA DRY KISS).

This module centralizes:
- Raw timing constants
- Edge proximity adaptation (duration / stretch / depth / tilt / yaw adjustments)
- Shared easing aliases
- Lightweight helper structs so main `card.rs` stays lean

The goal: `card.rs` focuses only on:
  - Reading props / signals
  - Picking a variant
  - Wiring keyframes

While this module provides pure, side‑effect free computation of the
numbers used to build those keyframes.

Additions should justify themselves in comments (focus on WHY over WHAT).
*/

/// Perspective baseline (cm) for settled card (matches existing visual baseline)
pub const CARD_PERSPECTIVE_CM: f64 = 80.0;
/// Perspective used during appear (slightly “flatter” for stronger depth illusion)
pub const CARD_APPEAR_PERSPECTIVE_CM: f64 = 60.0;

/// Maximum Gaussian blur applied at sequence start (enter/appear) or end (leave)
pub const CARD_MAX_BLUR: f32 = 1.0;

/// FPS: only switch between normal and reduced
pub const NORMAL_FPS: f64 = 40.0;
pub const REDUCED_FPS: f64 = 8.0;

// Base timing constants (ms) for each animation type
pub const CARD_APPEAR_BASE_MS: f64 = 1800.0;
pub const CARD_ENTER_BASE_MS: f64 = 1600.0;
pub const CARD_LEAVE_BASE_MS: f64 = 1450.0;

/// Normalization distance (px) used to compute an edge proximity ratio (0..=1)
pub const CARD_EDGE_NORM_DISTANCE_PX: f32 = 600.0;

/// Extra stretch added (beyond requested) at max edge proximity
pub const CARD_EDGE_STRETCH_EXTRA: f32 = 0.35;
/// Extra depth (|z|) scaling at max edge proximity
pub const CARD_EDGE_DEPTH_FACTOR: f32 = 0.60;
/// Fraction of tilt removed at max edge proximity (less flip near edge)
pub const CARD_EDGE_TILT_REDUCTION: f32 = 0.40;
/// Fraction of yaw removed at max edge proximity (enter)
pub const CARD_EDGE_YAW_REDUCTION_ENTER: f32 = 0.35;
/// Fraction of yaw removed at max edge proximity (leave)
pub const CARD_EDGE_YAW_REDUCTION_LEAVE: f32 = 0.30;

/// Scale overshoot (enter) – additional scale on X axis proportional to edge ratio
pub const CARD_ENTER_X_OVERSHOOT: f32 = 0.05;
/// Scale shrink (leave) – both axes proportional to edge ratio
pub const CARD_LEAVE_SCALE_SHRINK: f32 = 0.04;

/// Cubic curve timing points (equal intervals for smooth cubic motion)
pub const CARD_CURVE_POINT1_T: f64 = 0.33; // First curve point
pub const CARD_CURVE_POINT2_T: f64 = 0.67; // Second curve point

/// Stretch effects for motion blur (always >= 1.0, never compression)
pub const CARD_ENTER_STRETCH_START: f32 = 1.15; // Initial stretch on entry
pub const CARD_LEAVE_STRETCH_END: f32 = 1.25; // Final stretch on leave

/// Output of edge adaptation pass for appear animation
#[derive(Debug, Clone, Copy)]
pub struct AppearEdgeAdapt {
    pub edge_ratio: f32,
    pub final_stretch_factor: f32,
    pub adjusted_tilt_deg: f32,
    pub adjusted_from_z: f32,
    pub curve_point1_t: f64,
    pub curve_point2_t: f64,
    pub total_t: f64,
    pub perspective_scale: f64,
}

/// Output of edge adaptation for enter travel animation
#[derive(Debug, Clone, Copy)]
pub struct EnterEdgeAdapt {
    pub edge_ratio: f32,
    pub adjusted_from_rot_y_deg: f32,
    pub adjusted_from_z: f32,
    pub curve_point1_t: f64,
    pub curve_point2_t: f64,
    pub total_t: f64,
    pub perspective_scale: f64,
    pub entry_stretch_start: f32,
}

/// Output of edge adaptation for leave travel animation
#[derive(Debug, Clone, Copy)]
pub struct LeaveEdgeAdapt {
    pub edge_ratio: f32,
    pub adjusted_to_rot_y_deg: f32,
    pub adjusted_to_z: f32,
    pub curve_point1_t: f64,
    pub curve_point2_t: f64,
    pub total_t: f64,
    pub perspective_scale: f64,
    pub leave_stretch_end: f32,
}

/// Compute a normalized edge proximity ratio (0..=1)
#[inline]
pub fn edge_ratio(distance_px: f32) -> f32 {
    (distance_px / CARD_EDGE_NORM_DISTANCE_PX).clamp(0.0, 1.0)
}

/// Adapt appear animation parameters based on starting displacement.
pub fn adapt_appear(
    from_x_px: f32,
    from_y_px: f32,
    from_z_px: f32,
    base_tilt_deg: f32,
    base_stretch: f32,
) -> AppearEdgeAdapt {
    let dist = from_x_px.abs().max(from_y_px.abs());
    let er = edge_ratio(dist);
    let final_stretch_factor = base_stretch + CARD_EDGE_STRETCH_EXTRA * er;
    let adjusted_tilt_deg = base_tilt_deg * (1.0 - CARD_EDGE_TILT_REDUCTION * er);
    let adjusted_from_z = from_z_px * (1.0 + CARD_EDGE_DEPTH_FACTOR * er);
    let curve_point1_t = CARD_APPEAR_BASE_MS * CARD_CURVE_POINT1_T;
    let curve_point2_t = CARD_APPEAR_BASE_MS * CARD_CURVE_POINT2_T;
    let total_t = CARD_APPEAR_BASE_MS;
    let perspective_scale = 0.85 - 0.25 * er as f64;

    AppearEdgeAdapt {
        edge_ratio: er,
        final_stretch_factor,
        adjusted_tilt_deg,
        adjusted_from_z,
        curve_point1_t,
        curve_point2_t,
        total_t,
        perspective_scale,
    }
}

/// Adapt enter travel animation.
pub fn adapt_enter(from_x_px: f32, from_z_px: f32, from_rot_y_deg: f32) -> EnterEdgeAdapt {
    let dist = from_x_px.abs();
    let er = edge_ratio(dist);
    let adjusted_from_rot_y_deg = from_rot_y_deg * (1.0 - CARD_EDGE_YAW_REDUCTION_ENTER * er);
    let adjusted_from_z = from_z_px * (1.0 + CARD_EDGE_DEPTH_FACTOR * er);
    let curve_point1_t = CARD_ENTER_BASE_MS * CARD_CURVE_POINT1_T;
    let curve_point2_t = CARD_ENTER_BASE_MS * CARD_CURVE_POINT2_T;
    let total_t = CARD_ENTER_BASE_MS;
    let perspective_scale = 0.95 - 0.30 * er as f64;
    let entry_stretch_start = CARD_ENTER_STRETCH_START + CARD_ENTER_X_OVERSHOOT * er;

    EnterEdgeAdapt {
        edge_ratio: er,
        adjusted_from_rot_y_deg,
        adjusted_from_z,
        curve_point1_t,
        curve_point2_t,
        total_t,
        perspective_scale,
        entry_stretch_start,
    }
}

/// Adapt leave travel animation.
pub fn adapt_leave(to_x_px: f32, to_z_px: f32, to_rot_y_deg: f32) -> LeaveEdgeAdapt {
    let dist = to_x_px.abs();
    let er = edge_ratio(dist);
    let adjusted_to_rot_y_deg = to_rot_y_deg * (1.0 - CARD_EDGE_YAW_REDUCTION_LEAVE * er);
    let adjusted_to_z = to_z_px * (1.0 + CARD_EDGE_DEPTH_FACTOR * er);
    let curve_point1_t = CARD_LEAVE_BASE_MS * CARD_CURVE_POINT1_T;
    let curve_point2_t = CARD_LEAVE_BASE_MS * CARD_CURVE_POINT2_T;
    let total_t = CARD_LEAVE_BASE_MS;
    let perspective_scale = 0.95 - 0.25 * er as f64;
    let leave_stretch_end = CARD_LEAVE_STRETCH_END + CARD_LEAVE_SCALE_SHRINK * er;

    LeaveEdgeAdapt {
        edge_ratio: er,
        adjusted_to_rot_y_deg,
        adjusted_to_z,
        curve_point1_t,
        curve_point2_t,
        total_t,
        perspective_scale,
        leave_stretch_end,
    }
}
