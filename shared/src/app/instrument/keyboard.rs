use super::config::Config;
use crate::geometry::Rect;
use hecs::{Bundle, Entity, World};

#[derive(Bundle)]
pub struct Track {
    pub left_hand: bool,
    pub rect: Rect,
}

impl Track {
    pub fn spawn(
        world: &mut World,
        config: &Config,
        left_hand: bool,
        button_rect: &Rect,
    ) -> Entity {
        let sign = if left_hand { -1.0 } else { 1.0 };
        let button_track_margin = config.button_size * 0.2 * sign;
        let track_length = (config.breadth * 2.0 + button_track_margin * sign + config.button_size) * sign;
        let track_breadth = (config.button_size + button_track_margin * 2.0 * sign) * sign;

        let side = if left_hand {
            button_rect.bottom_right()
        } else {
            button_rect.top_left()
        };

        let rect = if config.portrait {
            Rect::new(
                side.x - button_track_margin,
                side.x - button_track_margin + track_length,
                side.y - button_track_margin,
                side.y - button_track_margin + track_breadth,
            )
        } else {
            Rect::new(
                side.x - button_track_margin,
                side.x - button_track_margin + track_breadth,
                side.y - button_track_margin,
                side.y - button_track_margin + track_length,
            )
        };

        world.spawn((Self { rect, left_hand },))
    }
}

#[derive(Bundle)]
pub struct Button {
    pub track: Entity,
    pub rect: Rect,
    pub group_button: (usize, usize),
}

impl Button {
    pub fn spawn(world: &mut World, config: &Config, group: usize, button: usize) -> Entity {
        let button_space_b = (config.breadth - config.button_size) / 2.0;
        let button_space_l = (config.length / (config.groups * config.buttons_group) as f32
            - config.button_size)
            / 2.0;
        let side = config.breadth + button_space_b;
        let side_breadth = side + config.button_size;
        let idx = (group - 1) * config.buttons_group + (button - 1);
        let main = (config.button_size + button_space_l * 2.0) * idx as f32 + button_space_l;
        let main_length = main + config.button_size;
        let rect = if config.portrait {
            Rect::new(side, side_breadth, main, main_length)
        } else {
            Rect::new(main, main_length, side, side_breadth)
        };

        let track = Track::spawn(world, config, group % 2 == 0, &rect);
        world.spawn((Button {
            rect,
            track,
            group_button: (group, button),
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
        let group_length = config.length / config.groups as f32;
        let rect = if config.portrait {
            Rect::new(
                config.breadth,
                config.breadth * 2.0,
                group_length * (group - 1) as f32,
                group_length * group as f32,
            )
        } else {
            Rect::new(
                group_length * (group - 1) as f32,
                group_length * group as f32,
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
