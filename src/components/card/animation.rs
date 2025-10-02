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

/// Scale overshoot (enter) – additional scale on X axis proportional to edge ratio
pub const CARD_ENTER_X_OVERSHOOT: f32 = 0.05;
/// Scale shrink (leave) – both axes proportional to edge ratio
pub const CARD_LEAVE_SCALE_SHRINK: f32 = 0.04;

/// Stretch effects for motion blur (always >= 1.0, never compression)
pub const CARD_ENTER_STRETCH_START: f32 = 1.15; // Initial stretch on entry
pub const CARD_LEAVE_STRETCH_END: f32 = 1.25; // Final stretch on leave

/// 3-keyframe enter animation settling constants
pub const CARD_ENTER_SETTLE_TIME_RATIO: f64 = 0.7; // Settle keyframe at 70% of total duration
pub const CARD_ENTER_POSITION_OVERSHOOT: f32 = 0.05; // 5% position overshoot in opposite direction
pub const CARD_ENTER_SCALE_OVERSHOOT: f32 = 1.02; // 2% scale overshoot for natural motion
pub const CARD_ENTER_Z_SETTLE_RATIO: f32 = 0.2; // Z-depth at settle (fraction of start depth)
pub const CARD_ENTER_ROTATION_OVERSHOOT: f32 = 1.1; // 10% rotation overshoot
pub const CARD_ENTER_BLUR_SETTLE_RATIO: f32 = 0.3; // Blur remaining at settle keyframe
pub const CARD_ENTER_PERSPECTIVE_SETTLE: f64 = 0.98; // Perspective scale at settle

/// Y-axis rotation for sideways card entry/exit during frame 2→3 transition
pub const CARD_SIDEWAYS_ROTATION_DEG: f32 = -90.0; // Rotate to -90° for sideways motion

/// Scale effects for off-screen positions
pub const CARD_OFFSCREEN_HORIZONTAL_STRETCH: f32 = 1.4; // Horizontal stretch at off-screen positions
pub const CARD_OFFSCREEN_VERTICAL_SQUEEZE: f32 = 0.7; // Vertical squeeze at off-screen positions

/// Proportional small scale for intermediate animation state
pub const CARD_INTERMEDIATE_SMALL_SCALE: f32 = 0.85; // Proportionally small scale for intermediate frames

/// Output of edge adaptation for enter travel animation
#[derive(Debug, Clone, Copy)]
pub struct EnterEdgeAdapt {
    pub edge_ratio: f32,
    pub adjusted_from_rot_y_deg: f32,
    pub adjusted_from_z: f32,
    pub perspective_scale: f64,
    pub entry_stretch_start: f32,
}

/// Output of edge adaptation for leave travel animation
#[derive(Debug, Clone, Copy)]
pub struct LeaveEdgeAdapt {
    pub edge_ratio: f32,
    pub adjusted_to_rot_y_deg: f32,
    pub adjusted_to_z: f32,
    pub perspective_scale: f64,
    pub leave_stretch_end: f32,
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
    let entry_stretch_start = CARD_ENTER_STRETCH_START + CARD_ENTER_X_OVERSHOOT * er;

    EnterEdgeAdapt {
        edge_ratio: er,
        adjusted_from_rot_y_deg,
        adjusted_from_z,
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
    let perspective_scale = 0.95 - 0.25 * er as f64;
    let leave_stretch_end = CARD_LEAVE_STRETCH_END + CARD_LEAVE_SCALE_SHRINK * er;

    LeaveEdgeAdapt {
        edge_ratio: er,
        adjusted_to_rot_y_deg,
        adjusted_to_z,
        perspective_scale,
        leave_stretch_end,
    }
}
