use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq)]
pub struct Config {
    pub portrait: bool,
    pub width: f32,
    pub height: f32,
    pub breadth: f32,
    pub length: f32,
    pub groups: usize,
    pub buttons_group: usize,
    pub button_size: f32,
}

impl Eq for Config {}

impl Config {
    pub fn new(width: f32, height: f32) -> Self {
        let portrait = height > width;

        let (length, breadth) = if portrait {
            (height, width / 3.0)
        } else {
            (width, height / 3.0)
        };

        let bs_size_b = breadth * 0.6;
        let bs_size_l = 932.0 / 6.0 * length / 932.0;

        let button_size = bs_size_b.min(bs_size_l);

        let max_buttons = (length / button_size).floor() as usize;

        let slots = max_buttons.div_euclid(2);

        let groups_2 = slots.div_euclid(2);
        let groups_3 = slots.div_euclid(3);
        let groups_5 = slots.div_euclid(5);

        let (groups, buttons_group) = if groups_5 > 1 {
            (groups_5, 5)
        } else if groups_3 > 1 {
            (groups_3, 3)
        } else {
            (groups_2, 2)
        };

        Config {
            portrait,
            width,
            height,
            length,
            breadth,
            button_size,
            groups: groups.try_into().unwrap_or(1),
            buttons_group,
        }
    }
}
