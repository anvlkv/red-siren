/*!
 * Intro animation configuration constants.
 *
 * This centralizes all tunable parameters for the intro <-> instrument
 * CSS animations so we can iterate without touching generation logic.
 *
 * Phases (aligned with docs/animation.md):
 *
 *   Keyframe 1   (PHASE_START_PCT)           : All elements collapsed on sun
 *   Keyframe 1.5 (PHASE_FADE_OVERSHOOT_PCT)  : Picture fading, bands overshoot
 *   Keyframe 2   (PHASE_COLLAPSE_LINE_PCT)   : Collapse onto main axis (first key line)
 *   Keyframe 3   (PHASE_TRAVEL_PCT)          : Travel outward toward natural layout
 *   Keyframe 4   (PHASE_REFINE_PCT)          : Band geometric / padding refinement
 *   End          (100%)                      : Relaxed (natural DOM positions)
 *
 * Only collapsed starting positions are computed; the relaxed end
 * state is the natural (transform: none) layout position.
 *
 * Reverse animation uses the same keyframes with animation-direction: reverse.
 *
 * Composition (picture) fade is only needed during forward animation.
 * We hide the composition (remove from DOM / conditional <Show>) using
 * a timer at INTRO_HIDE_AT_FRACTION * INTRO_ANIM_DURATION_MS.
 */

// -----------------------------------------------------------------------------
// Intro artwork reference (moved from layout; kept front & center here)
// -----------------------------------------------------------------------------
/// Width of the static intro composition artwork viewBox
pub const INTRO_ART_WIDTH: f32 = 430.0;
/// Height of the static intro composition artwork viewBox
pub const INTRO_ART_HEIGHT: f32 = 932.0;
/// Sun center X within artwork space
pub const INTRO_SUN_CX: f32 = 107.0;
/// Sun center Y within artwork space
pub const INTRO_SUN_CY: f32 = 164.0;

/// Forward animation total duration (ms)
pub const INTRO_ANIM_DURATION_MS: u32 = 1600;

/// Reverse animation duration (keep symmetrical for now)
pub const INTRO_REVERSE_DURATION_MS: u32 = INTRO_ANIM_DURATION_MS;

/// Fraction (0.0..=1.0) of forward duration when we hide the intro composition
///
/// (Matches requirement to use a timer; 0.60 means at 60% progress)
pub const INTRO_HIDE_AT_FRACTION: f32 = 0.60;

// -----------------------------------------------------------------------------
// Phase percentage markers (0.0 ..= 100.0)
// -----------------------------------------------------------------------------

pub const PHASE_START_PCT: f32 = 0.0;
pub const PHASE_FADE_OVERSHOOT_PCT: f32 = 15.0;
pub const PHASE_COLLAPSE_LINE_PCT: f32 = 40.0;
pub const PHASE_TRAVEL_PCT: f32 = 70.0;
pub const PHASE_REFINE_PCT: f32 = 85.0;

/// Picture is fully faded out by this percentage (aligned with Keyframe 2)
pub const PICTURE_FADE_OUT_PCT: f32 = PHASE_COLLAPSE_LINE_PCT;

// -----------------------------------------------------------------------------
// Scaling & shape parameters
// -----------------------------------------------------------------------------

/// Initial scale for keys while collapsed on the sun cluster
pub const KEY_START_SCALE: f32 = 0.25;

/// Keys do not overshoot by default (can raise later)
pub const KEY_OVERSHOOT_SCALE: f32 = 1.00;

/// Initial scale for bands while collapsed
pub const BAND_START_SCALE: f32 = 0.25;

/// Band overshoot scale at Keyframe 1.5
pub const BAND_OVERSHOOT_SCALE: f32 = 1.30;

/// Band scale during axis collapse (Keyframe 2)
pub const BAND_AXIS_COLLAPSE_SCALE: f32 = 1.00;

/// Band scale during refinement & final
pub const BAND_REFINE_SCALE: f32 = 1.00;

/// Border-radius (px) when band is fully circular (applied using a large value)
pub const BAND_CIRCLE_RADIUS_PX: f32 = 9999.0;

// -----------------------------------------------------------------------------
// Strings
// -----------------------------------------------------------------------------

/// Starting rotation (deg) of strings in intro composition
pub const STRING_START_ROT_DEG: f32 = -17.1246;

/// Final rotation (deg) of strings (aligned with instrument layout)
pub const STRING_END_ROT_DEG: f32 = 0.0;

// -----------------------------------------------------------------------------
// Easing definitions
// -----------------------------------------------------------------------------

/// Main easing for element motion (keys / bands / strings)
pub const EASE_MAIN: &str = "cubic-bezier(.65,.1,.35,1)";

/// Easing for picture fade (can be softer)
pub const EASE_FADE: &str = "ease-in";

/// Easing hint for reverse direction if specialized later
pub const EASE_REVERSE: &str = "cubic-bezier(.35,.1,.65,1)";

// -----------------------------------------------------------------------------
// Utility helpers
// -----------------------------------------------------------------------------

/// Convert a percentage (0.0..=100.0) of the forward intro animation
/// into a whole number of milliseconds.
#[inline]
pub fn percent_to_time_ms(pct: f32) -> u32 {
    let clamped = pct.clamp(0.0, 100.0);
    ((clamped / 100.0) * INTRO_ANIM_DURATION_MS as f32).round() as u32
}

/// Milliseconds (forward) at which to hide the intro composition node.
#[inline]
pub fn hide_intro_at_ms() -> u32 {
    (INTRO_HIDE_AT_FRACTION * INTRO_ANIM_DURATION_MS as f32).round() as u32
}
