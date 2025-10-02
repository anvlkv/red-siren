use keyframe::CanTween;
use mint::{Point2, Vector2};
use common::orientation::LayoutOrientation;

use crate::util::animation::{tween_tuple_vectors, tween_vectors};

#[derive(Debug, Clone, PartialEq)]
/// Desired background animation
pub enum IntroAnimationTarget {
    /// All content pages (Home, About, Donate, Permissions)
    Intro,
    /// Tuner page
    Tuner {
        layout: common::tuner::Layout,
        data: common::tuner::Data,
    },
    /// Play page
    Instrument(common::instrument::Layout),
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct IntroAnimationState {
    pub view_box: (Point2<f32>, Point2<f32>),
    pub picture_opacity: f32,
}

impl Default for IntroAnimationState {
    fn default() -> Self {
        todo!()
        // Self {
        //     view_box: (Point2 { x: 0.0, y: 0.0 }, Point2 { x: 430.0, y: 932.0 }),
        //     picture_opacity: 1.0,
        //     suns_positions: vec![Point2 { x: 107.0, y: 164.0 }],
        //     suns_splits: vec![Vector2 { x: 0.0, y: 0.0 }],
        //     sun_radius: 39.0,
        //     keybands_positions: Vec::default(),
        //     string_1_position: (
        //         Point2 {
        //             x: 73.7113,
        //             y: 576.054,
        //         },
        //         Point2 {
        //             x: 73.7113 + 53.653,
        //             y: 576.054,
        //         },
        //     ),
        //     string_2_position: (
        //         Point2 {
        //             x: 73.7113,
        //             y: 576.054 + 8.25253,
        //         },
        //         Point2 {
        //             x: 73.7113 + 53.653,
        //             y: 576.054 + 8.25253,
        //         },
        //     ),
        //     strings_rotation: (
        //         -17.1246,
        //         Point2 {
        //             x: 48.3365,
        //             y: 585.964,
        //         },
        //     ),
        //     strings_stroke: 2.0,
        // }
    }
}

impl From<IntroAnimationTarget> for IntroAnimationState {
    fn from(value: IntroAnimationTarget) -> Self {
        todo!()
        // match value {
        //     IntroAnimationTarget::Intro => Self::default(),
        //     IntroAnimationTarget::Tuner { .. } => todo!(),
        //     IntroAnimationTarget::Instrument(layout) => {
        //         log::debug!("Converting IntroAnimationTarget::Instrument to IntroAnimationState");
        //         log::debug!("Layout: {:?}", layout);

        //         let total_keys =
        //             layout.num_groups.get() as usize * layout.num_keys_per_group.get() as usize;
        //         let num_groups = layout.num_groups.get() as usize;
        //         let keys_per_group = layout.num_keys_per_group.get() as usize;

        //         log::debug!(
        //             "Total keys: {}, Groups: {}, Keys per group: {}",
        //             total_keys,
        //             num_groups,
        //             keys_per_group
        //         );

        //         // Calculate sun positions (keys) and keyband positions
        //         let mut suns_positions = Vec::with_capacity(total_keys);
        //         let mut keybands_positions = Vec::with_capacity(total_keys);

        //         // Determine main and cross axes based on orientation
        //         let (main_axis, cross_axis) = match layout.orientation {
        //             LayoutOrientation::Horizontal => (layout.space.x, layout.space.y),
        //             LayoutOrientation::Vertical => (layout.space.y, layout.space.x),
        //         };

        //         // Calculate available space after safe area padding
        //         let available_main =
        //             main_axis - layout.safe_area_padding[0] - layout.safe_area_padding[2];
        //         let available_cross =
        //             cross_axis - layout.safe_area_padding[1] - layout.safe_area_padding[3];

        //         // Calculate group spacing
        //         let total_groups_gap = layout.groups_gap * (num_groups - 1) as f32;
        //         let group_main_size = (available_main - total_groups_gap) / num_groups as f32;

        //         // Calculate key spacing within groups
        //         let total_key_gaps = layout.key_bands_gap * (keys_per_group - 1) as f32;
        //         let key_cross_size = (available_cross - total_key_gaps) / keys_per_group as f32;

        //         for group_idx in 0..num_groups {
        //             let group_main_offset = layout.safe_area_padding[0]
        //                 + (group_main_size + layout.groups_gap) * group_idx as f32;

        //             for key_idx in 0..keys_per_group {
        //                 let key_cross_offset = layout.safe_area_padding[1]
        //                     + (key_cross_size + layout.key_bands_gap) * key_idx as f32;

        //                 // Calculate position based on orientation
        //                 let (x, y) = match layout.orientation {
        //                     LayoutOrientation::Horizontal => (
        //                         group_main_offset + group_main_size * 0.5,
        //                         key_cross_offset + key_cross_size * 0.5,
        //                     ),
        //                     LayoutOrientation::Vertical => (
        //                         key_cross_offset + key_cross_size * 0.5,
        //                         group_main_offset + group_main_size * 0.5,
        //                     ),
        //                 };

        //                 suns_positions.push(Point2 { x, y });

        //                 // Calculate keyband positions (full-rounded rectangles around keys)
        //                 let band_start = Point2 {
        //                     x: x - layout.key_band_length * 0.5,
        //                     y: y - layout.key_band_breadth * 0.5,
        //                 };
        //                 let band_end = Point2 {
        //                     x: x + layout.key_band_length * 0.5,
        //                     y: y + layout.key_band_breadth * 0.5,
        //                 };
        //                 keybands_positions.push((band_start, band_end));
        //             }
        //         }

        //         log::debug!("Generated {} sun positions", suns_positions.len());
        //         log::debug!("Generated {} keyband positions", keybands_positions.len());
        //         if !keybands_positions.is_empty() {
        //             log::debug!("First keyband: {:?}", keybands_positions[0]);
        //             if keybands_positions.len() > 1 {
        //                 log::debug!(
        //                     "Last keyband: {:?}",
        //                     keybands_positions[keybands_positions.len() - 1]
        //                 );
        //             }
        //         }
        //         log::debug!(
        //             "View box: ({}, {}) to ({}, {})",
        //             0.0,
        //             0.0,
        //             layout.space.x,
        //             layout.space.y
        //         );
        //         log::debug!(
        //             "String 1 (left) position: {:?}",
        //             layout.left_string_position
        //         );
        //         log::debug!(
        //             "String 2 (right) position: {:?}",
        //             layout.right_string_position
        //         );
        //         log::debug!(
        //             "Default string 1 position: {:?}",
        //             Self::default().string_1_position
        //         );
        //         log::debug!(
        //             "Default string 2 position: {:?}",
        //             Self::default().string_2_position
        //         );

        //         Self {
        //             view_box: (
        //                 Point2 { x: 0.0, y: 0.0 },
        //                 Point2 {
        //                     x: layout.space.x,
        //                     y: layout.space.y,
        //                 },
        //             ),
        //             picture_opacity: 0.0,
        //             suns_positions,
        //             suns_splits: (0..total_keys)
        //                 .map(|_| Vector2 { x: 0.0, y: 0.0 })
        //                 .collect(), // splits not used
        //             sun_radius: layout.key_radius,
        //             keybands_positions,
        //             string_1_position: layout.left_string_position,
        //             string_2_position: layout.right_string_position,
        //             strings_rotation: (0.0, Point2 { x: 0.0, y: 0.0 }), // rotation not specified, use default
        //             strings_stroke: 1.0,
        //         }
        //     }
        // }
    }
}

impl CanTween for IntroAnimationState {
    fn ease(from: Self, to: Self, time: impl keyframe::num_traits::Float) -> Self {
        todo!()
        // let suns_positions = tween_vectors(&from.suns_positions, &to.suns_positions, time);
        // let suns_splits = tween_vectors(&from.suns_splits, &to.suns_splits, time);
        // let keybands_positions =
        //     tween_tuple_vectors(&from.keybands_positions, &to.keybands_positions, time);

        // Self {
        //     view_box: (
        //         CanTween::ease(from.view_box.0, to.view_box.0, time),
        //         CanTween::ease(from.view_box.1, to.view_box.1, time),
        //     ),
        //     suns_positions,
        //     suns_splits,
        //     keybands_positions,
        //     sun_radius: CanTween::ease(from.sun_radius, to.sun_radius, time),
        //     picture_opacity: CanTween::ease(from.picture_opacity, to.picture_opacity, time),
        //     string_1_position: (
        //         CanTween::ease(from.string_1_position.0, to.string_1_position.0, time),
        //         CanTween::ease(from.string_1_position.1, to.string_1_position.1, time),
        //     ),
        //     string_2_position: (
        //         CanTween::ease(from.string_2_position.0, to.string_2_position.0, time),
        //         CanTween::ease(from.string_2_position.1, to.string_2_position.1, time),
        //     ),
        //     strings_rotation: (
        //         CanTween::ease(from.strings_rotation.0, to.strings_rotation.0, time),
        //         CanTween::ease(from.strings_rotation.1, to.strings_rotation.1, time),
        //     ),
        //     strings_stroke: CanTween::ease(from.strings_stroke, to.strings_stroke, time),
        // }
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use keyframe::{keyframes, AnimationSequence};
//     use shared::instrument::layout::layout_test_cases;

//     #[test]
//     fn standard_instrument_layouts() {
//         let initial = IntroAnimationState::default();

//         for l in layout_test_cases() {
//             let target = IntroAnimationTarget::Instrument(l);
//             let state: IntroAnimationState = target.into();
//             println!("created state for layout: {}x{}", l.space.x, l.space.y);

//             let mut sequence = keyframes![(initial.clone(), 0.0), (state, 1.0)];
//             println!("created keyframes");

//             _ = sequence.advance_to(0.5);
//             println!("advance sequence, 50%");

//             let result = sequence.now();
//             println!("data at 50%: {result:?}");
//         }
//     }

//     #[test]
//     fn regression_intro_keybands_transition_midway() {
//         // Simulate a transition from default (no keybands_positions) to a state with keybands
//         let from_state = IntroAnimationState {
//             keybands_positions: vec![],
//             ..IntroAnimationState::default()
//         };
//         let to_state = IntroAnimationState {
//             keybands_positions: vec![
//                 (Point2 { x: 0.0, y: 0.0 }, Point2 { x: 10.0, y: 10.0 }),
//                 (Point2 { x: 20.0, y: 20.0 }, Point2 { x: 30.0, y: 30.0 }),
//             ],
//             ..IntroAnimationState::default()
//         };

//         let mid = IntroAnimationState::ease(from_state.clone(), to_state.clone(), 0.5f32);

//         assert!(
//             mid.keybands_positions.len() <= to_state.keybands_positions.len(),
//             "Mid animation keybands count should not exceed target ({:?} vs {:?})",
//             mid.keybands_positions.len(),
//             to_state.keybands_positions.len()
//         );
//     }
// }
