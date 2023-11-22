use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq)]
pub struct Config {
    pub portrait: bool,
    pub width: f64,
    pub height: f64,
    pub breadth: f64,
    pub length: f64,
    pub groups: usize,
    pub buttons_group: usize,
    pub button_size: f64,
    pub button_track_margin: f64,
    pub safe_area: [f64; 4],
}

impl Eq for Config {}

impl Config {
    pub fn new(width: f64, height: f64, dpi: f64, safe_area: [f64; 4]) -> Self {
        let portrait = height > width;
        
        let (length, breadth, ratio) = if portrait {
            (height, width / 3.0, height / width)
        } else {
            (width, height / 3.0, width / height)
        };

        log::debug!(
            "config width: {}; height: {}; dpi: {}; ratio: {}; insets: {:?}",
            width,
            height,
            dpi,
            ratio,
            safe_area
        );

        let button_size = (breadth / ratio) * match dpi {
            _ => 0.75
        };

        let max_buttons = (length / button_size).floor() as usize;

        let slots = max_buttons.div_euclid(2);

        let (groups, buttons_group) = vec![
            (slots.div_euclid(5), 5),
            (slots.div_euclid(3), 3),
            (slots.div_euclid(2), 2),
        ]
        .into_iter()
        .fold((1, 1), |acc, (groups, buttons_group)| {
            if groups * buttons_group > acc.0 * acc.1 || (groups > acc.0 && acc.0 == 1) {
                (groups, buttons_group)
            }
            else {
                acc
            }
        });

        Config {
            portrait,
            width,
            height,
            length,
            breadth,
            button_size,
            groups,
            buttons_group,
            button_track_margin: 0.2,
            safe_area
        }
    }
}
