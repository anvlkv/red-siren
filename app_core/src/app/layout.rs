use anyhow::Result;
use euclid::default::{Box2D, Point2D, SideOffsets2D, Size2D};
use hecs::{Entity, World};

use crate::{ObjectStyle, Objects};

use super::{
    config::Config,
    objects::{Object, ObjectBuilder, Shapes},
};

#[derive(Default)]
pub struct Layout {
    pub buttons: Vec<Entity>,
    pub left_strings: Vec<Entity>,
    pub right_strings: Vec<Entity>,
    pub tracks: Vec<Entity>,
}

impl Layout {
    pub fn new(config: &Config, world: &mut World) -> Result<Self> {
        let mut left_strings = vec![];
        let mut right_strings = vec![];
        let mut tracks = vec![];
        let mut buttons = vec![];

        // strings_config
        let is_stereo = config.groups > 1;

        // buttons config
        let button_space_side = (config.safe_breadth - config.button_size) / 2.0;
        let button_space_main = (config.active_length
            / (config.groups * config.buttons_group) as f64
            - config.button_size)
            / 2.0;
        let side = config.safe_breadth + button_space_side;
        let side_breadth = side + config.button_size;

        // track config
        let button_track_margin = config.button_size * config.button_track_margin;
        let track_length = config.safe_breadth * 2.0 + button_track_margin + config.button_size;

        for gx in 0..config.groups {
            let left_hand = (gx + 1) % 2 == 0;

            for bx in 0..config.buttons_group {
                let idx = gx * config.buttons_group + bx;
                let button_obj =
                    Self::make_layout_button(config, idx, button_space_main, side, side_breadth)?;

                let button_rect = button_obj.shape.containing_rect();

                let button = world.spawn((button_obj,));
                buttons.push(button);

                let track_obj = Self::make_layout_track(
                    config,
                    button_rect,
                    track_length,
                    button_track_margin,
                    left_hand,
                )?;

                let track = world.spawn((track_obj,));

                tracks.push(track);

                let string_obj = Self::make_layout_string(config, left_hand)?;
                let string = world.spawn((string_obj,));

                if left_hand {
                    left_strings.push(string)
                } else {
                    right_strings.push(string)
                }

                if !is_stereo {
                    let string_obj = Self::make_layout_string(config, !left_hand)?;
                    let string = world.spawn((string_obj,));

                    if !left_hand {
                        left_strings.push(string)
                    } else {
                        right_strings.push(string)
                    }
                }
            }
        }

        Ok(Self {
            buttons,
            left_strings,
            right_strings,
            tracks,
        })
    }

    pub fn make_objects(&self, world: &mut World, dark: bool) -> Objects {
        let mut inner = vec![];

        inner.extend(
            self.left_strings
                .iter()
                .map(|e| Objects::make_paint(world, e, dark, ObjectStyle::StringLine(1))),
        );
        inner.extend(
            self.right_strings
                .iter()
                .map(|e| Objects::make_paint(world, e, dark, ObjectStyle::StringLine(1))),
        );
        inner.extend(
            self.tracks
                .iter()
                .map(|e| Objects::make_paint(world, e, dark, ObjectStyle::InstrumentTrack)),
        );
        inner.extend(
            self.buttons
                .iter()
                .map(|e| Objects::make_paint(world, e, dark, ObjectStyle::InstrumentButton)),
        );

        Objects(inner)
    }

    fn make_layout_button(
        config: &Config,
        idx: usize,
        button_space_main: f64,
        side: f64,
        side_breadth: f64,
    ) -> Result<Object> {
        let offset = if config.portrait {
            config.safe_area[1]
        } else {
            config.safe_area[0]
        } + config.whitespace;

        let main = offset
            + (config.button_size + button_space_main * 2.0) * idx as f64
            + button_space_main;
        let main_length = main + config.button_size;

        let rect = if config.portrait {
            Box2D::new(
                Point2D::new(side, main),
                Point2D::new(side_breadth, main_length),
            )
        } else {
            Box2D::new(
                Point2D::new(main, side),
                Point2D::new(main_length, side_breadth),
            )
        };

        let object = ObjectBuilder::default()
            .shape(Shapes::Circle(rect))
            .build()?;

        Ok(object)
    }

    fn make_layout_track(
        config: &Config,
        button_rect: Box2D<f64>,
        track_length: f64,
        button_track_margin: f64,
        left_hand: bool,
    ) -> Result<Object> {
        let mut offsets = SideOffsets2D::<f64>::zero();

        if config.portrait {
            offsets += SideOffsets2D::new(button_track_margin, 0_f64, button_track_margin, 0_f64);
        } else {
            offsets += SideOffsets2D::new(0_f64, button_track_margin, 0_f64, button_track_margin);
        }

        if left_hand {
            if config.portrait {
                offsets += SideOffsets2D::new(0_f64, button_track_margin, 0_f64, track_length);
            } else {
                offsets += SideOffsets2D::new(button_track_margin, 0_f64, track_length, 0_f64);
            }
        } else {
            if config.portrait {
                offsets += SideOffsets2D::new(0_f64, track_length, 0_f64, button_track_margin);
            } else {
                offsets += SideOffsets2D::new(track_length, 0_f64, button_track_margin, 0_f64);
            }
        }

        let rect = button_rect.outer_box(offsets);
        let rounding = Size2D::new(
            (config.button_size + button_track_margin * 2.0) / 2.0,
            (config.button_size + button_track_margin * 2.0) / 2.0,
        );

        let track_obj = ObjectBuilder::default()
            .shape(Shapes::RoundedRect(rect, rounding))
            .build()?;
        Ok(track_obj)
    }

    fn make_layout_string(config: &Config, left_hand: bool) -> Result<Object> {
        let c = if left_hand { 1.0 } else { 2.0 };

        let path = if config.portrait {
            let x = (config.width - config.safe_breadth) / c;
            let y_end = config.active_length
                + config.safe_area[3]
                + config.safe_area[1]
                + config.whitespace * 2.0;
            vec![Point2D::new(x, 0_f64), Point2D::new(x, y_end)]
        } else {
            let y = (config.height - config.safe_breadth) / c;
            let x_end = config.active_length
                + config.safe_area[2]
                + config.safe_area[0]
                + config.whitespace * 2.0;
            vec![Point2D::new(0_f64, y), Point2D::new(x_end, y)]
        };

        let string_obj = ObjectBuilder::default()
            .shape(Shapes::Path {
                p0: path.first().cloned().unwrap(),
                p1: path.last().cloned().unwrap(),
                path,
            })
            .build()?;

        Ok(string_obj)
    }
}
