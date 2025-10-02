/*! Core data types for Card animation. Keep lean and focused. */

use keyframe_derive::CanTween;

/// Low‑level interpolated transform/effect state for the card.
///
/// Each animation constructs keyframes (AnimationSequence<CardEffects>)
/// that lerp these fields over time. Keep this lean; additions here
/// increase per-frame work.
#[derive(Debug, Default, Clone, Copy, CanTween)]
pub struct CardEffects {
    pub x_px: f32,
    pub y_px: f32,
    pub z_px: f32,
    pub rot_x_deg: f32,
    pub rot_y_deg: f32,
    pub scale_x: f32,
    pub scale_y: f32,
    pub perspective: f64,
    pub blur: f32,
}

/// Edge anchor used by edge-based enter/leave animations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeSide {
    Top,
    Bottom,
    Left,
    Right,
}

/// Animation variants (kept lean; higher-level code maps scenarios to these).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CardAnimation {
    EnterTravel3D {
        from_x_px: f32,
        from_z_px: f32,
        from_rot_y_deg: f32,
        to_rot_y_deg: f32,
    },
    LeaveTravel3D {
        to_x_px: f32,
        to_z_px: f32,
        to_rot_y_deg: f32,
    },
    /// Perpendicular edge entry (CompactMenu, pages, etc.)
    EdgeEnter3D {
        side: EdgeSide,
        offset_px: f32,
        depth_z_px: f32,
        yaw_deg: f32,
    },
    /// Perpendicular edge exit.
    EdgeLeave3D {
        side: EdgeSide,
        offset_px: f32,
        depth_z_px: f32,
        yaw_deg: f32,
    },
}

impl Eq for CardAnimation {}
