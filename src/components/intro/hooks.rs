use std::collections::HashMap;

use leptos::prelude::*;

/*
 * Intro animation hooks
 *
 * These hooks expose per-element animation style strings and controls for:
 * - keys
 * - bands
 * - strings (left / right)
 * - animation direction (forward / reverse)
 * - rerun trigger (to restart animations)
 *
 * Context Expectations
 * --------------------
 * A parent (the `Intro` component) must `provide_context` with an
 * `IntroAnimationContext` instance after constructing all animation
 * keyframes and the inline style maps.
 *
 * Philosophy (MAYA DRY KISS)
 * --------------------------
 * Keep these hooks *thin*: they only retrieve and wrap memoized strings.
 * No business logic, no computations. All generation belongs to the
 * animation builder inside the `Intro` component.
 */

/// Context provided by the `Intro` component after animation build.
#[derive(Clone)]
pub struct IntroAnimationContext {
    /// Map of (group, key) -> full inline style string for key animation
    pub key_styles: Memo<HashMap<(u8, u8), String>>,
    /// Map of (group, key) -> full inline style string for band animation
    pub band_styles: Memo<HashMap<(u8, u8), String>>,
    /// Inline style for the left string animation
    pub left_string_style: Memo<String>,
    /// Inline style for the right string animation
    pub right_string_style: Memo<String>,
    /// Current animation direction:
    /// false = forward (intro -> instrument)
    /// true  = reverse (instrument -> intro)
    pub direction_reverse: RwSignal<bool>,
    /// Rerun counter – increment to force a rebuild / retrigger pass if
    /// the parent elects to rebuild style strings on change.
    pub rerun_counter: RwSignal<u32>,
}

/// Internal helper: unwrap context or panic (developer error if missing).
pub fn expect_intro_anim_ctx() -> IntroAnimationContext {
    expect_context::<IntroAnimationContext>()
}

/// Get a signal with the animation style for a specific key (g,k).
///
/// Returns empty string if the style map isn't populated yet.
pub fn use_key_anim_style(g: u8, k: u8) -> Signal<String> {
    let IntroAnimationContext { key_styles, .. } = expect_intro_anim_ctx();
    Memo::new(move |_| {
        key_styles()
            .get(&(g, k))
            .cloned()
            .unwrap_or_else(String::new)
    })
    .into()
}

/// Get a signal with the animation style for a specific band (g,k).
///
/// Returns empty string if the style map isn't populated yet.
pub fn use_band_anim_style(g: u8, k: u8) -> Signal<String> {
    let IntroAnimationContext { band_styles, .. } = expect_intro_anim_ctx();
    Memo::new(move |_| {
        band_styles()
            .get(&(g, k))
            .cloned()
            .unwrap_or_else(String::new)
    })
    .into()
}

/// Style accessor for the left string animation.
pub fn use_left_string_anim_style() -> Signal<String> {
    let IntroAnimationContext {
        left_string_style, ..
    } = expect_intro_anim_ctx();
    left_string_style.into()
}

/// Style accessor for the right string animation.
pub fn use_right_string_anim_style() -> Signal<String> {
    let IntroAnimationContext {
        right_string_style, ..
    } = expect_intro_anim_ctx();
    right_string_style.into()
}

/// Read & write animation direction (reverse flag).
///
/// Returns (getter, setter).
pub fn use_intro_anim_direction() -> (Signal<bool>, impl Fn(bool) + Copy) {
    let IntroAnimationContext {
        direction_reverse, ..
    } = expect_intro_anim_ctx();
    (direction_reverse.into(), move |rev: bool| {
        direction_reverse.set(rev)
    })
}

/// Provides a callback that, when invoked, increments the rerun counter.
///
/// Parent logic can watch this counter to rebuild animation styles in
/// order to force a retrigger (e.g. after toggling direction with the
/// same final values).
pub fn use_intro_anim_rerun_trigger() -> impl Fn() {
    let IntroAnimationContext { rerun_counter, .. } = expect_intro_anim_ctx();
    move || {
        rerun_counter.update(|c| *c = c.wrapping_add(1));
    }
}

/// Convenience helper: toggle direction & trigger an animation rerun
/// in one call.
///
/// Useful when implementing a reverse transition button / navigation.
pub fn use_intro_reverse_and_rerun() -> impl Fn() {
    let IntroAnimationContext {
        direction_reverse,
        rerun_counter,
        ..
    } = expect_intro_anim_ctx();
    move || {
        direction_reverse.update(|d| *d = !*d);
        rerun_counter.update(|c| *c = c.wrapping_add(1));
    }
}
