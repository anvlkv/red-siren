use keyframe::CanTween;
use mint::{Point2, Vector2};
use shared::orientation::LayoutOrientation;

use crate::util::animation::{tween_tuple_vectors, tween_vectors};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntroAnimationTarget {
    Intro,
    Tuner,
    Instrument(shared::instrument::Layout),
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct IntroAnimationState {
    pub view_box: (Point2<f32>, Point2<f32>),
    pub picture_opacity: f32,
    pub suns_positions: Vec<Point2<f32>>,
    pub suns_splits: Vec<Vector2<f32>>,
    pub sun_radius: f32,
    pub keybands_positions: Vec<(Point2<f32>, Point2<f32>)>,
    pub string_1_position: (Point2<f32>, Point2<f32>),
    pub string_2_position: (Point2<f32>, Point2<f32>),
    pub strings_rotation: (f32, Point2<f32>),
    pub strings_stroke: f32,
}

impl Default for IntroAnimationState {
    fn default() -> Self {
        Self {
            view_box: (Point2 { x: 0.0, y: 0.0 }, Point2 { x: 430.0, y: 932.0 }),
            picture_opacity: 1.0,
            suns_positions: vec![Point2 { x: 107.0, y: 164.0 }],
            suns_splits: vec![Vector2 { x: 0.0, y: 0.0 }],
            sun_radius: 39.0,
            keybands_positions: Vec::default(),
            string_1_position: (
                Point2 {
                    x: 73.7113,
                    y: 576.054,
                },
                Point2 {
                    x: 73.7113 + 53.653,
                    y: 576.054,
                },
            ),
            string_2_position: (
                Point2 {
                    x: 73.7113,
                    y: 576.054 + 8.25253,
                },
                Point2 {
                    x: 73.7113 + 53.653,
                    y: 576.054 + 8.25253,
                },
            ),
            strings_rotation: (
                -17.1246,
                Point2 {
                    x: 48.3365,
                    y: 585.964,
                },
            ),
            strings_stroke: 2.0,
        }
    }
}

impl From<IntroAnimationTarget> for IntroAnimationState {
    fn from(value: IntroAnimationTarget) -> Self {
        match value {
            IntroAnimationTarget::Intro => Self::default(),
            IntroAnimationTarget::Tuner => todo!(),
            IntroAnimationTarget::Instrument(layout) => {
                let total_keys =
                    layout.num_groups.get() as usize * layout.num_keys_per_group.get() as usize;
                let num_groups = layout.num_groups.get() as usize;
                let keys_per_group = layout.num_keys_per_group.get() as usize;

                // Calculate sun positions (keys) and keyband positions
                let mut suns_positions = Vec::with_capacity(total_keys);
                let mut keybands_positions = Vec::with_capacity(total_keys);

                // Determine main and cross axes based on orientation
                let (main_axis, cross_axis) = match layout.orientation {
                    LayoutOrientation::Horizontal => (layout.space.x, layout.space.y),
                    LayoutOrientation::Vertical => (layout.space.y, layout.space.x),
                };

                // Calculate available space after safe area padding
                let available_main =
                    main_axis - layout.safe_area_padding[0] - layout.safe_area_padding[2];
                let available_cross =
                    cross_axis - layout.safe_area_padding[1] - layout.safe_area_padding[3];

                // Calculate group spacing
                let total_groups_gap = layout.groups_gap * (num_groups - 1) as f32;
                let group_main_size = (available_main - total_groups_gap) / num_groups as f32;

                // Calculate key spacing within groups
                let total_key_gaps = layout.key_bands_gap * (keys_per_group - 1) as f32;
                let key_cross_size = (available_cross - total_key_gaps) / keys_per_group as f32;

                for group_idx in 0..num_groups {
                    let group_main_offset = layout.safe_area_padding[0]
                        + (group_main_size + layout.groups_gap) * group_idx as f32;

                    for key_idx in 0..keys_per_group {
                        let key_cross_offset = layout.safe_area_padding[1]
                            + (key_cross_size + layout.key_bands_gap) * key_idx as f32;

                        // Calculate position based on orientation
                        let (x, y) = match layout.orientation {
                            LayoutOrientation::Horizontal => (
                                group_main_offset + group_main_size * 0.5,
                                key_cross_offset + key_cross_size * 0.5,
                            ),
                            LayoutOrientation::Vertical => (
                                key_cross_offset + key_cross_size * 0.5,
                                group_main_offset + group_main_size * 0.5,
                            ),
                        };

                        suns_positions.push(Point2 { x, y });

                        // Calculate keyband positions (full-rounded rectangles around keys)
                        let band_start = Point2 {
                            x: x - layout.key_band_length * 0.5,
                            y: y - layout.key_band_breadth * 0.5,
                        };
                        let band_end = Point2 {
                            x: x + layout.key_band_length * 0.5,
                            y: y + layout.key_band_breadth * 0.5,
                        };
                        keybands_positions.push((band_start, band_end));
                    }
                }

                Self {
                    view_box: (
                        Point2 { x: 0.0, y: 0.0 },
                        Point2 {
                            x: layout.space.x,
                            y: layout.space.y,
                        },
                    ),
                    picture_opacity: 0.0,
                    suns_positions,
                    suns_splits: (0..total_keys)
                        .map(|_| Vector2 { x: 0.0, y: 0.0 })
                        .collect(), // splits not used
                    sun_radius: layout.key_radius,
                    keybands_positions,
                    string_1_position: layout.left_string_position,
                    string_2_position: layout.right_string_position,
                    strings_rotation: (0.0, Point2 { x: 0.0, y: 0.0 }), // rotation not specified, use default
                    strings_stroke: 1.0,
                }
            }
        }
    }
}

impl CanTween for IntroAnimationState {
    fn ease(from: Self, to: Self, time: impl keyframe::num_traits::Float) -> Self {
        let suns_positions = tween_vectors(&from.suns_positions, &to.suns_positions, time);
        let suns_splits = tween_vectors(&from.suns_splits, &to.suns_splits, time);
        let keybands_positions =
            tween_tuple_vectors(&from.keybands_positions, &to.keybands_positions, time);

        Self {
            view_box: (
                CanTween::ease(from.view_box.0, to.view_box.0, time),
                CanTween::ease(from.view_box.1, to.view_box.1, time),
            ),
            suns_positions,
            suns_splits,
            keybands_positions,
            sun_radius: CanTween::ease(from.sun_radius, to.sun_radius, time),
            picture_opacity: CanTween::ease(from.picture_opacity, to.picture_opacity, time),
            string_1_position: (
                CanTween::ease(from.string_1_position.0, to.string_1_position.0, time),
                CanTween::ease(from.string_1_position.1, to.string_1_position.1, time),
            ),
            string_2_position: (
                CanTween::ease(from.string_2_position.0, to.string_2_position.0, time),
                CanTween::ease(from.string_2_position.1, to.string_2_position.1, time),
            ),
            strings_rotation: (
                CanTween::ease(from.strings_rotation.0, to.strings_rotation.0, time),
                CanTween::ease(from.strings_rotation.1, to.strings_rotation.1, time),
            ),
            strings_stroke: CanTween::ease(from.strings_stroke, to.strings_stroke, time),
        }
    }
}
