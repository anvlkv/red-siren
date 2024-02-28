use std::collections::BTreeMap;


use hecs::{Bundle, Entity, World};
use serde::{Deserialize, Serialize};

const MIN_BUTTON_SIZE_IN: f64 = 0.75;
const MAX_BUTTON_SIZE_B_RATIO: f64 = 0.6;
const BUTTON_TRACK_MARGIN_RATIO: f64 = 0.2;
const BUTTON_SPACE_RATIO: f64 = 2.0;
const F_BASE: f64 = 110.0;
const F_MAX: f64 = 5500.0;


#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq, Bundle)]
pub struct Config {
    pub portrait: bool,
    pub width: f64,
    pub height: f64,
    pub breadth: f64,
    pub length: f64,
    pub whitespace: f64,
    pub groups: usize,
    pub buttons_group: usize,
    pub n_buttons: usize,
    pub button_size: f64,
    pub button_track_margin: f64,
    pub safe_area: [f64; 4],
    pub f0: f32,
}

impl Eq for Config {}

impl Config {
    pub fn configs_for_screen(width: f64, height: f64, dpi: f64, safe_area: [f64; 4]) -> Vec<Self> {
        let portrait = height > width;

        let (length, safe_length, safe_breadth) = if portrait {
            (
                height,
                height - safe_area[1] - safe_area[3],
                (width / 3.0).min(width - safe_area[0] - safe_area[2]),
            )
        } else {
            (
                width,
                width - safe_area[0] - safe_area[2],
                (height / 3.0).min(height - safe_area[1] - safe_area[3]),
            )
        };

        let max_button_size = (safe_breadth * MAX_BUTTON_SIZE_B_RATIO).round() as usize;
        let min_button_size = f64::sqrt(dpi * MIN_BUTTON_SIZE_IN).round() as usize;

        let (min_groups, min_buttons) = {
            let max_buttons = ((length - max_button_size as f64 * BUTTON_SPACE_RATIO)
                / max_button_size as f64)
                .round() as usize;
            let slots = max_buttons.div_euclid(2);
            vec![
                (slots.div_euclid(5), 5),
                (slots.div_euclid(3), 3),
                (slots.div_euclid(2), 2),
            ]
            .into_iter()
            .fold((1, 1), |acc, (groups, buttons_group)| {
                if groups * buttons_group > acc.0 * acc.1
                    || groups * buttons_group == acc.0 * acc.1 && buttons_group > acc.1
                {
                    (groups, buttons_group)
                } else {
                    acc
                }
            })
        };

        let min_count = min_groups * min_buttons;

        let f_c = (F_BASE / f64::sqrt((length * safe_breadth) / dpi)) as f32;

        let f0 = {
            let mut f0 = f_c;

            while f0 < F_BASE as f32 {
                f0 *= 2.0;
            }

            f0
        };

        let mut candidates = Vec::<Self>::new();

        for size in min_button_size..=max_button_size {
            let space = size as f64 * BUTTON_SPACE_RATIO * 2.0;
            let active_length = (safe_length - space).round();
            let slots = num_integer::gcd(space.round() as usize + size, active_length as usize);
            for (groups, buttons_group) in [
                (slots.div_euclid(5), 5),
                (slots.div_euclid(3), 3),
                (slots.div_euclid(2), 2),
            ] {
                let count = groups * buttons_group;
                let used_space = space * count as f64;
                let f_max = f0 as f64 * 2.0 * (groups * buttons_group) as f64;
                if count >= min_count && used_space < safe_length && f_max <= F_MAX {
                    let whitespace = (safe_length - active_length) / 2.0;

                    candidates.push(Self {
                        portrait,
                        width,
                        height,
                        length: active_length,
                        breadth: safe_breadth,
                        button_size: size as f64,
                        groups,
                        buttons_group,
                        n_buttons: buttons_group * groups,
                        button_track_margin: BUTTON_TRACK_MARGIN_RATIO,
                        safe_area,
                        whitespace,
                        f0,
                    });
                }
            }
        }

        Self::rate_sorted(candidates)
    }

    fn rate_sorted(candidates: Vec<Self>) -> Vec<Self> {
        let (d_size, d_groups, d_buttons_group, d_active_length, d_count) = candidates.iter().fold(
            (
                (0_f64, 0_f64),
                (0_usize, 0_usize),
                (0_usize, 0_usize),
                (0_f64, 0_f64),
                (0_usize, 0_usize),
            ),
            |mut acc, config| {
                let count = config.groups * config.buttons_group;
                acc.0 = (
                    acc.0 .0.min(config.button_size),
                    acc.0 .1.max(config.button_size),
                );
                acc.1 = (acc.1 .0.min(config.groups), acc.1 .1.max(config.groups));
                acc.2 = (
                    acc.2 .0.min(config.buttons_group),
                    acc.2 .1.max(config.buttons_group),
                );
                acc.3 = (acc.3 .0.min(config.length), acc.3 .1.max(config.length));
                acc.4 = (acc.4 .0.min(count), acc.4 .1.max(count));
                acc
            },
        );

        let (d_size, d_groups, d_buttons_group, d_active_length, d_count) = (
            (d_size.1 - d_size.0),
            (d_groups.1 - d_groups.0),
            (d_buttons_group.1 - d_buttons_group.0),
            (d_active_length.1 - d_active_length.0),
            (d_count.1 - d_count.0).max(1),
        );

        let candidates = BTreeMap::<usize, Self>::from_iter(candidates.into_iter().map(|config| {
            let count = (config.buttons_group * config.groups) as f64;
            let score = (config.button_size as f64 / d_size
                + config.groups as f64 / d_groups as f64
                + config.buttons_group as f64 / d_buttons_group as f64
                + config.length as f64 / d_active_length)
                * (1.0 + d_count as f64 / count);

            (score.round() as usize, config)
        }));

        candidates.into_values().collect()
    }
}
