/*!
Card types module.

This isolates the core data structures used by the Card animation system
so that `card.rs` can stay focused on rendering concerns and
`animation.rs` can focus on numeric adaptation & easing decisions.

MAYA DRY KISS:
- Small, purpose‑fit types
- No logic / side effects here
- Re-export friendly so other modules can depend without pulling in heavy code
*/

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
    pub tilt_x_deg: f32,
    pub rot_y_deg: f32,
    pub scale_x: f32,
    pub scale_y: f32,
    pub perspective: f64,
    pub blur: f32,
}

/// Axis along which a stretch overshoot is applied for Appear3D.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StretchAxis {
    X,
    Y,
}

/// Declarative animation variants consumed by the Card component.
///
/// Variants are intentionally high‑level: all edge / distance adaptations
/// are computed elsewhere (animation.rs) before keyframes are built.
#[derive(Debug, Clone, Copy)]
pub enum CardAnimation {
    /// First-time or contextual appear with depth + stretch.
    Appear3D {
        from_x_px: f32,
        from_y_px: f32,
        from_z_px: f32,
        from_tilt_x_deg: f32,
        stretch_axis: StretchAxis,
        stretch_factor: f32,
    },
    /// Travel in from an off‑screen (left/back) origin with yaw easing to 0.
    EnterTravel3D {
        from_x_px: f32,
        from_z_px: f32,
        from_rot_y_deg: f32,
        to_rot_y_deg: f32,
    },
    /// Travel out toward an off‑screen (right/back) destination with yaw.
    LeaveTravel3D {
        to_x_px: f32,
        to_z_px: f32,
        to_rot_y_deg: f32,
    },
}
