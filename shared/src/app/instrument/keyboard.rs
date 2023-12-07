use super::config::Config;
use crate::geometry::Rect;
use hecs::{Bundle, Entity, World};

#[derive(Bundle)]
pub struct Track {
    pub left_hand: bool,
    pub rect: Rect,
    pub freq: (f32, f32),
}

impl Track {
    pub fn spawn(
        world: &mut World,
        config: &Config,
        left_hand: bool,
        button_rect: &Rect,
        base_freq: f32,
        max_freq: f32,
    ) -> Entity {
        let button_track_margin = config.button_size * config.button_track_margin;

        let track_length = config.breadth * 2.0 + button_track_margin + config.button_size;

        let rect = if config.portrait {
            button_rect
                .offset_top(button_track_margin)
                .offset_bottom(button_track_margin)
        } else {
            button_rect
                .offset_left(button_track_margin)
                .offset_right(button_track_margin)
        };

        let rect = if left_hand {
            if config.portrait {
                rect.offset_left_and_right(track_length, button_track_margin)
            } else {
                rect.offset_top_and_bottom(button_track_margin, track_length)
            }
        } else if config.portrait {
            rect.offset_left_and_right(button_track_margin, track_length)
        } else {
            rect.offset_top_and_bottom(track_length, button_track_margin)
        };

        let freq = (base_freq, max_freq);

        world.spawn((Self {
            rect,
            left_hand,
            freq,
        },))
    }
}

#[derive(Bundle)]
pub struct Button {
    pub track: Entity,
    pub rect: Rect,
    pub group_button: (usize, usize),
    pub f_n: usize,
    pub freq: f32,
}

impl Button {
    pub fn spawn(world: &mut World, config: &Config, group: usize, button: usize) -> Entity {
        let button_space_side = (config.breadth - config.button_size) / 2.0;
        let button_space_main = (config.length / (config.groups * config.buttons_group) as f64
            - config.button_size)
            / 2.0;
        let total_buttons = config.buttons_group * config.groups;
        let idx = (group - 1) * config.buttons_group + (button - 1);

        let side = config.breadth + button_space_side;
        let side_breadth = side + config.button_size;

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
            Rect::new(side, side_breadth, main, main_length)
        } else {
            Rect::new(main, main_length, side, side_breadth)
        };

        let f_n = total_buttons - idx;
        let freq = config.f0 * (f_n * 2) as f32 - config.f0;
        let max_freq = freq + config.f0;

        let track = Track::spawn(world, config, group % 2 == 0, &rect, freq, max_freq);

        world.spawn((Button {
            rect,
            track,
            group_button: (group, button),
            f_n,
            freq,
        },))
    }
}

#[derive(Bundle)]
pub struct ButtonGroup {
    pub buttons: Vec<Entity>,
    pub rect: Rect,
}

impl ButtonGroup {
    pub fn spawn(world: &mut World, config: &Config, group: usize) -> Entity {
        let mut buttons = vec![];
        for j in 1..=config.buttons_group {
            buttons.push(Button::spawn(world, config, group, j));
        }
        let group_length = config.length / config.groups as f64;
        let rect = if config.portrait {
            Rect::new(
                config.breadth,
                config.breadth * 2.0,
                group_length * (group - 1) as f64 + config.safe_area[1] + config.whitespace,
                group_length * group as f64,
            )
        } else {
            Rect::new(
                group_length * (group - 1) as f64 + config.safe_area[0] + config.whitespace,
                group_length * group as f64,
                config.breadth,
                config.breadth * 2.0,
            )
        };

        world.spawn((Self { buttons, rect },))
    }
}

#[derive(Bundle)]
pub struct Keyboard {
    pub groups: Vec<Entity>,
    pub rect: Rect,
}

impl Keyboard {
    pub fn spawn(world: &mut World, config: &Config) -> Entity {
        let mut groups = vec![];
        for i in 1..=config.groups {
            groups.push(ButtonGroup::spawn(world, config, i));
        }
        let rect = Rect::size(config.width, config.height);
        world.spawn((Keyboard { groups, rect },))
    }
}
