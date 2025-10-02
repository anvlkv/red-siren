use keyframe::functions::{EaseIn, EaseInOut, EaseOut};
use keyframe::{keyframes, AnimationSequence};

use crate::components::intro::IntroAnimationState;

// pub const INTRO_TO_INSTRUMENT_MS: f64 = 1500.0;
// pub const INTRO_TO_TUNER_MS: f64 = 1000.0;

// pub const INSTRUMENT_TO_INTRO_MS: f64 = 700.0;
// pub const TUNER_TO_INTRO_MS: f64 = 500.0;

// pub const INSTRUMENT_TO_TUNER_MS: f64 = 400.0;
// pub const TUNER_TO_INSTRUMENT_MS: f64 = 300.0;

// /// Default static (no animation) keyframe sequence
// pub fn default_static() -> AnimationSequence<IntroAnimationState> {
//     let state = IntroAnimationState::default();
//     keyframes![(state, 0.0)]
// }

// /// Animate transition from intro to instrument
// /// Stages:
// /// 1. Fade out picture (0-20%)
// /// 2. Expand suns to positions (20-60%)
// /// 3. Reveal keybands and adjust strings (60-100%)
// pub fn intro_to_instrument(
//     from_state: IntroAnimationState,
//     to_state: IntroAnimationState,
// ) -> AnimationSequence<IntroAnimationState> {
//     log::debug!("intro_to_instrument animation started");
//     log::debug!(
//         "From keybands: {}, To keybands: {}",
//         from_state.keybands_positions.len(),
//         to_state.keybands_positions.len()
//     );

//     // Stage 1: Just fade picture
//     let stage_1 = IntroAnimationState {
//         picture_opacity: to_state.picture_opacity,
//         ..from_state.clone()
//     };

//     // Stage 2: Move suns to positions, start string adjustments
//     let stage_2 = IntroAnimationState {
//         suns_positions: to_state.suns_positions.clone(),
//         sun_radius: to_state.sun_radius,
//         string_1_position: (
//             // Interpolate halfway
//             [
//                 from_state.string_1_position.0.x + to_state.string_1_position.0.x * 0.3,
//                 from_state.string_1_position.0.y + to_state.string_1_position.0.y * 0.3,
//             ]
//             .into(),
//             [
//                 from_state.string_1_position.1.x + to_state.string_1_position.1.x * 0.3,
//                 from_state.string_1_position.1.y + to_state.string_1_position.1.y * 0.3,
//             ]
//             .into(),
//         ),
//         string_2_position: (
//             [
//                 from_state.string_2_position.0.x + to_state.string_2_position.0.x * 0.3,
//                 from_state.string_2_position.0.y + to_state.string_2_position.0.y * 0.3,
//             ]
//             .into(),
//             [
//                 from_state.string_2_position.1.x + to_state.string_2_position.1.x * 0.3,
//                 from_state.string_2_position.1.y + to_state.string_2_position.1.y * 0.3,
//             ]
//             .into(),
//         ),
//         strings_rotation: (
//             from_state.strings_rotation.0 * 0.7,
//             from_state.strings_rotation.1,
//         ),
//         ..stage_1.clone()
//     };

//     log::debug!(
//         "Stage 2: {} suns, {} keybands",
//         stage_2.suns_positions.len(),
//         stage_2.keybands_positions.len()
//     );

//     // Stage 3: Expand view, reveal keybands
//     let stage_3 = IntroAnimationState {
//         view_box: to_state.view_box,
//         keybands_positions: to_state.keybands_positions.clone(),
//         strings_stroke: to_state.strings_stroke,
//         ..stage_2.clone()
//     };

//     log::debug!(
//         "Stage 3: {} suns, {} keybands",
//         stage_3.keybands_positions.len(),
//         to_state.keybands_positions.len()
//     );

//     keyframes![
//         (from_state, 0.0, EaseIn),
//         (stage_1, 0.2 * INTRO_TO_INSTRUMENT_MS, EaseOut),
//         (stage_2, 0.6 * INTRO_TO_INSTRUMENT_MS, EaseOut),
//         (stage_3, 0.85 * INTRO_TO_INSTRUMENT_MS, EaseInOut),
//         (to_state, INTRO_TO_INSTRUMENT_MS)
//     ]
// }

// /// Animate transition **back** from instrument to intro
// /// Stages:
// /// 1. Hide keybands (0-20%)
// /// 2. Collapse suns back to center (20-70%)
// /// 3. Fade picture back in and reset strings (70-100%)
// pub fn instrument_to_intro(
//     from_state: IntroAnimationState,
//     to_state: IntroAnimationState,
// ) -> AnimationSequence<IntroAnimationState> {
//     log::debug!("instrument_to_intro animation started");
//     log::debug!(
//         "From keybands: {}, To keybands: {}",
//         from_state.keybands_positions.len(),
//         to_state.keybands_positions.len()
//     );

//     // Stage 1: Hide keybands, start view adjustment
//     let stage_1 = IntroAnimationState {
//         keybands_positions: vec![],
//         view_box: (
//             [
//                 from_state.view_box.0.x * 0.8 + to_state.view_box.0.x * 0.2,
//                 from_state.view_box.0.y * 0.8 + to_state.view_box.0.y * 0.2,
//             ]
//             .into(),
//             [
//                 from_state.view_box.1.x * 0.8 + to_state.view_box.1.x * 0.2,
//                 from_state.view_box.1.y * 0.8 + to_state.view_box.1.y * 0.2,
//             ]
//             .into(),
//         ),
//         ..from_state.clone()
//     };

//     // Stage 2: Move suns back, adjust strings
//     let stage_2 = IntroAnimationState {
//         suns_positions: to_state.suns_positions.clone(),
//         sun_radius: to_state.sun_radius,
//         string_1_position: to_state.string_1_position,
//         string_2_position: to_state.string_2_position,
//         strings_rotation: to_state.strings_rotation,
//         strings_stroke: to_state.strings_stroke,
//         view_box: to_state.view_box,
//         ..stage_1.clone()
//     };

//     keyframes![
//         (from_state, 0.0, EaseIn),
//         (stage_1, 0.2 * INSTRUMENT_TO_INTRO_MS, EaseIn),
//         (stage_2, 0.7 * INSTRUMENT_TO_INTRO_MS, EaseOut),
//         (to_state, INSTRUMENT_TO_INTRO_MS)
//     ]
// }

// /// Animate transition from intro to tuner
// /// Stages:
// /// 1. Fade picture and adjust view (0-30%)
// /// 2. Position strings for tuning (30-70%)
// /// 3. Final adjustments (70-100%)
// pub fn intro_to_tuner(
//     from_state: IntroAnimationState,
//     to_state: IntroAnimationState,
// ) -> AnimationSequence<IntroAnimationState> {
//     log::debug!("intro_to_tuner animation started");

//     // Stage 1: Fade picture, start view change
//     let stage_1 = IntroAnimationState {
//         picture_opacity: to_state.picture_opacity,
//         view_box: (
//             [
//                 from_state.view_box.0.x * 0.5 + to_state.view_box.0.x * 0.5,
//                 from_state.view_box.0.y * 0.5 + to_state.view_box.0.y * 0.5,
//             ]
//             .into(),
//             [
//                 from_state.view_box.1.x * 0.5 + to_state.view_box.1.x * 0.5,
//                 from_state.view_box.1.y * 0.5 + to_state.view_box.1.y * 0.5,
//             ]
//             .into(),
//         ),
//         ..from_state.clone()
//     };

//     // Stage 2: Position strings and suns for tuning
//     let stage_2 = IntroAnimationState {
//         string_1_position: to_state.string_1_position,
//         string_2_position: to_state.string_2_position,
//         strings_rotation: (0.0, to_state.strings_rotation.1),
//         strings_stroke: to_state.strings_stroke,
//         suns_positions: to_state.suns_positions.clone(),
//         sun_radius: to_state.sun_radius,
//         ..stage_1.clone()
//     };

//     keyframes![
//         (from_state, 0.0, EaseIn),
//         (stage_1, 0.3 * INTRO_TO_TUNER_MS, EaseOut),
//         (stage_2, 0.7 * INTRO_TO_TUNER_MS, EaseInOut),
//         (to_state, INTRO_TO_TUNER_MS)
//     ]
// }

// /// Animate transition **back** from tuner to intro
// /// Stages:
// /// 1. Reset strings (0-40%)
// /// 2. Restore view and fade picture in (40-100%)
// pub fn tuner_to_intro(
//     from_state: IntroAnimationState,
//     to_state: IntroAnimationState,
// ) -> AnimationSequence<IntroAnimationState> {
//     log::debug!("tuner_to_intro animation started");

//     // Stage 1: Reset strings and suns
//     let stage_1 = IntroAnimationState {
//         string_1_position: to_state.string_1_position,
//         string_2_position: to_state.string_2_position,
//         strings_rotation: to_state.strings_rotation,
//         suns_positions: to_state.suns_positions.clone(),
//         sun_radius: to_state.sun_radius,
//         ..from_state.clone()
//     };

//     keyframes![
//         (from_state, 0.0, EaseIn),
//         (stage_1, 0.4 * TUNER_TO_INTRO_MS, EaseInOut),
//         (to_state, TUNER_TO_INTRO_MS, EaseOut)
//     ]
// }

// /// Animate transition from instrument to tuner
// /// Stages:
// /// 1. Collapse keybands (0-30%)
// /// 2. Reposition for tuning (30-100%)
// pub fn instrument_to_tuner(
//     from_state: IntroAnimationState,
//     to_state: IntroAnimationState,
// ) -> AnimationSequence<IntroAnimationState> {
//     log::debug!("instrument_to_tuner animation started");
//     log::debug!("From keybands: {}", from_state.keybands_positions.len());

//     // Stage 1: Hide keybands quickly
//     let stage_1 = IntroAnimationState {
//         keybands_positions: vec![],
//         ..from_state.clone()
//     };

//     keyframes![
//         (from_state, 0.0, EaseIn),
//         (stage_1, 0.3 * INSTRUMENT_TO_TUNER_MS, EaseIn),
//         (to_state, INSTRUMENT_TO_TUNER_MS, EaseOut)
//     ]
// }

// /// Animate transition **back** from tuner to instrument
// /// Stages:
// /// 1. Position suns (0-50%)
// /// 2. Reveal keybands (50-100%)
// pub fn tuner_to_instrument(
//     from_state: IntroAnimationState,
//     to_state: IntroAnimationState,
// ) -> AnimationSequence<IntroAnimationState> {
//     log::debug!("tuner_to_instrument animation started");
//     log::debug!("To keybands: {}", to_state.keybands_positions.len());

//     // Stage 1: Position suns and adjust view
//     let stage_1 = IntroAnimationState {
//         suns_positions: to_state.suns_positions.clone(),
//         sun_radius: to_state.sun_radius,
//         view_box: to_state.view_box,
//         string_1_position: to_state.string_1_position,
//         string_2_position: to_state.string_2_position,
//         strings_rotation: to_state.strings_rotation,
//         ..from_state.clone()
//     };

//     keyframes![
//         (from_state, 0.0, EaseIn),
//         (stage_1, 0.5 * TUNER_TO_INSTRUMENT_MS, EaseOut),
//         (to_state, TUNER_TO_INSTRUMENT_MS, EaseOut)
//     ]
// }
