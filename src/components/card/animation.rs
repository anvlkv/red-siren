/*!
Card animation constants & helpers (MAYA DRY KISS).

This module centralizes:
- Raw timing constants
- Edge proximity adaptation (duration / stretch / depth / tilt / yaw adjustments)
- Custom cubic path easing functions for natural curved motion
- Lightweight helper structs so main `card.rs` stays lean

The goal: `card.rs` focuses only on:
  - Reading props / signals
  - Picking a variant
  - Wiring keyframes (now just 2 per animation!)

While this module provides pure, side‑effect free computation of the
numbers used to build those keyframes.

Additions should justify themselves in comments (focus on WHY over WHAT).
*/

/// Perspective baseline (cm) for settled card (matches existing visual baseline)
pub const CARD_PERSPECTIVE_CM: f64 = 80.0;

/// Maximum Gaussian blur applied at sequence start (enter) or end (leave)
pub const CARD_MAX_BLUR: f32 = 1.0;

// Base timing constants (ms) for each animation type

pub const CARD_ENTER_BASE_MS: f64 = 600.0;
pub const CARD_LEAVE_BASE_MS: f64 = 450.0;
pub const CARD_ENTER_SHORT_MS: f64 = 300.0;
pub const CARD_LEAVE_SHORT_MS: f64 = 200.0;

/// Normalization distance (px) used to compute an edge proximity ratio (0..=1)
pub const CARD_EDGE_NORM_DISTANCE_PX: f32 = 600.0;

/// Extra depth (|z|) scaling at max edge proximity
pub const CARD_EDGE_DEPTH_FACTOR: f32 = 0.60;

/// Fraction of yaw removed at max edge proximity (enter)
pub const CARD_EDGE_YAW_REDUCTION_ENTER: f32 = 0.35;
/// Fraction of yaw removed at max edge proximity (leave)
pub const CARD_EDGE_YAW_REDUCTION_LEAVE: f32 = 0.30;

// (removed unused scale overshoot & shrink constants)

// (removed unused stretch constants)

// (Removed unused settle/overshoot constants: simplified animations now use only start/end keyframes)

// (Removed unused sideways & scale variant constants for simplified animation)

/// Output of edge adaptation for enter travel animation
#[derive(Debug, Clone, Copy)]
pub struct EnterEdgeAdapt {
    pub edge_ratio: f32,
    pub adjusted_from_rot_y_deg: f32,
    pub adjusted_from_z: f32,
    pub perspective_scale: f64,
}

/// Output of edge adaptation for leave travel animation
#[derive(Debug, Clone, Copy)]
pub struct LeaveEdgeAdapt {
    pub edge_ratio: f32,
    pub adjusted_to_rot_y_deg: f32,
    pub adjusted_to_z: f32,
    pub perspective_scale: f64,
}

/// Compute a normalized edge proximity ratio (0..=1)
#[inline]
pub fn edge_ratio(distance_px: f32) -> f32 {
    (distance_px / CARD_EDGE_NORM_DISTANCE_PX).clamp(0.0, 1.0)
}

/// Adapt enter travel animation.
pub fn adapt_enter(from_x_px: f32, from_z_px: f32, from_rot_y_deg: f32) -> EnterEdgeAdapt {
    let dist = from_x_px.abs();
    let er = edge_ratio(dist);
    let adjusted_from_rot_y_deg = from_rot_y_deg * (1.0 - CARD_EDGE_YAW_REDUCTION_ENTER * er);
    let adjusted_from_z = from_z_px * (1.0 + CARD_EDGE_DEPTH_FACTOR * er);
    let perspective_scale = 0.95 - 0.30 * er as f64;

    EnterEdgeAdapt {
        edge_ratio: er,
        adjusted_from_rot_y_deg,
        adjusted_from_z,
        perspective_scale,
    }
}

/// Adapt leave travel animation.
pub fn adapt_leave(to_x_px: f32, to_z_px: f32, to_rot_y_deg: f32) -> LeaveEdgeAdapt {
    let dist = to_x_px.abs();
    let er = edge_ratio(dist);
    let adjusted_to_rot_y_deg = to_rot_y_deg * (1.0 - CARD_EDGE_YAW_REDUCTION_LEAVE * er);
    let adjusted_to_z = to_z_px * (1.0 + CARD_EDGE_DEPTH_FACTOR * er);
    let perspective_scale = 0.95 - 0.25 * er as f64;

    LeaveEdgeAdapt {
        edge_ratio: er,
        adjusted_to_rot_y_deg,
        adjusted_to_z,
        perspective_scale,
    }
}
