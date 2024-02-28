use anyhow::Result;
use ecolor::Rgba;
use euclid::default::{Box2D, Point2D, SideOffsets2D, Size2D};
use hecs::{Entity, World};

use super::{
    config::Config,
    objects::{Object, ObjectBuilder, Objects, Shapes, Stroke},
};

const RED: Rgba = Rgba::from_rgb(227_f32, 0_f32, 34_f32);
const BLACK: Rgba = Rgba::from_rgb(53_f32, 56_f32, 57_f32);
const GRAY: Rgba = Rgba::from_rgb(54_f32, 69_f32, 79_f32);
const CINNABAR: Rgba = Rgba::from_rgb(228_f32, 77_f32, 46_f32);

#[derive(Default)]
pub struct Layout {
    pub buttons: Vec<Entity>,
    pub strings: Vec<Entity>,
    pub tracks: Vec<Entity>,
    pub nodes: Vec<Entity>,
    pub objects: Objects,
}

impl Layout {
    pub fn layout(config: &Config, world: &mut World) -> Result<Self> {
        world.clear();

        let mut strings = vec![];
        let mut tracks = vec![];
        let mut buttons = vec![];
        let mut nodes = vec![];
        let mut objects = vec![];

        for i in [true, false] {
            let string_obj = Self::make_layout_string(config, i)?;
            let string = world.spawn((string_obj.clone(),));
            strings.push(string);
            objects.push((string, string_obj));
        }

        // buttons config
        let button_space_side = (config.breadth - config.button_size) / 2.0;
        let button_space_main = (config.length / (config.groups * config.buttons_group) as f64
            - config.button_size)
            / 2.0;
        let side = config.breadth + button_space_side;
        let side_breadth = side + config.button_size;

        // node config
        let stereo = config.groups >= 2;

        // track config
        let button_track_margin = config.button_size * config.button_track_margin;
        let track_length = config.breadth * 2.0 + button_track_margin + config.button_size;

        for gx in 0..config.groups {
            for bx in 0..config.buttons_group {
                let left_hand = (gx + 1) % 2 == 0;
                let idx = gx * config.buttons_group + bx;
                let button_obj =
                    Self::make_layout_button(config, idx, button_space_main, side, side_breadth)?;

                let button = world.spawn((button_obj.clone(),));

                buttons.push(button);

                let pan = if stereo {
                    if left_hand {
                        -0.95
                    } else {
                        0.95
                    }
                } else {
                    0_f32
                };

                let node_obj = Self::make_node(config, idx, button, pan)?;

                let node = world.spawn((node_obj.clone(),));

                nodes.push(node);

                let track_obj = Self::make_layout_track(
                    config,
                    button_obj.shape.containing_rect(),
                    track_length,
                    button_track_margin,
                    left_hand,
                )?;

                let track = world.spawn((track_obj.clone(),));

                tracks.push(track);

                objects.push((track, track_obj));
                objects.push((button, button_obj));
            }
        }

        Ok(Self {
            buttons,
            strings,
            tracks,
            nodes,
            objects: Objects(objects),
        })
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
            .fill(Some(RED))
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
            config.button_size + button_track_margin,
            config.button_size + button_track_margin,
        );

        let track_obj = ObjectBuilder::default()
            .shape(Shapes::RoundedRect(rect, rounding))
            .fill(Some(BLACK))
            .build()?;
        Ok(track_obj)
    }

    fn make_node(config: &Config, idx: usize, button: Entity, pan: f32) -> Result<au_core::Node> {
        let f_n = config.n_buttons - idx;
        let freq = config.f0 * (f_n * 2) as f32 - config.f0;
        let max_freq = freq + config.f0;

        let node_data = au_core::NodeDataBuilder::default()
            .button(button)
            .f_base(freq)
            .f_emit((freq, max_freq))
            .f_sense(((freq, max_freq), (0.0, 1.0)))
            .pan(pan)
            .build()?;

        Ok(node_data.into())
    }

    fn make_layout_string(config: &Config, inbound: bool) -> Result<Object> {
        let c = if inbound { 1.0 } else { 2.0 };

        let points = if config.portrait {
            let x = (config.width - config.breadth) / c;
            let y_end =
                config.length + config.safe_area[3] + config.safe_area[1] + config.whitespace * 2.0;
            vec![Point2D::new(x, 0_f64), Point2D::new(x, y_end)]
        } else {
            let y = (config.height - config.breadth) / c;
            let x_end =
                config.length + config.safe_area[2] + config.safe_area[0] + config.whitespace * 2.0;
            vec![Point2D::new(0_f64, y), Point2D::new(x_end, y)]
        };

        let string_obj = ObjectBuilder::default()
            .shape(Shapes::Path(points))
            .stroke(Some(Stroke {
                color: RED,
                width: 1.25,
            }))
            .build()?;

        Ok(string_obj)
    }
}
