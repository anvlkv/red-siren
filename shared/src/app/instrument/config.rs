use std::collections::{BTreeMap, HashSet};

use serde::{Deserialize, Serialize};

const MIN_BUTTON_SIZE_IN: f64 = 0.75;
const MAX_BUTTON_SIZE_B_RATIO: f64 = 0.6;
const BUTTON_TRACK_MARGIN_RATION: f64 = 0.2;
const BUTTON_SPACE_RATIO: f64 = 2.0;

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
            let max_buttons = ((length as f64 - max_button_size as f64 * BUTTON_SPACE_RATIO)
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

        let mut candidates = HashSet::<(usize, usize, usize, usize)>::new();

        for size in min_button_size..=max_button_size {
            let space = size as f64 * BUTTON_SPACE_RATIO * 2.0;
            let active_length = (safe_length - space).round() as usize;
            let slots = num_integer::gcd(space.round() as usize + size, active_length);
            for (groups, buttons_group) in vec![
                (slots.div_euclid(5), 5),
                (slots.div_euclid(3), 3),
                (slots.div_euclid(2), 2),
            ] {
                let count = groups * buttons_group;
                let used_space = space * count as f64;
                if count >= min_count && used_space < safe_length {
                    let _ = candidates.insert((size, groups, buttons_group, active_length));
                }
            }
        }

        let (d_size, d_groups, d_buttons_group, d_active_length, d_count) = candidates.iter().fold(
            ((0, 0), (0, 0), (0, 0), (0, 0), (0, 0)),
            |mut acc, (size, groups, buttons_group, active_length)| {
                acc.0 = (acc.0 .0.min(*size), acc.0 .1.max(*size));
                acc.1 = (acc.1 .0.min(*groups), acc.1 .1.max(*groups));
                acc.2 = (acc.2 .0.min(*buttons_group), acc.2 .1.max(*buttons_group));
                acc.3 = (acc.3 .0.min(*active_length), acc.3 .1.max(*active_length));
                let count = groups * buttons_group;
                acc.4 = (acc.4 .0.min(count), acc.4 .1.max(count));
                acc
            },
        );

        let (d_size, d_groups, d_buttons_group, d_active_length, d_count) = (
            (d_size.1 - d_size.0) as f64,
            (d_groups.1 - d_groups.0) as f64,
            (d_buttons_group.1 - d_buttons_group.0) as f64,
            (d_active_length.1 - d_active_length.0) as f64,
            (d_count.1 - d_count.0) as f64,
        );

        let candidates =
            BTreeMap::<usize, (usize, usize, usize, usize)>::from_iter(candidates.into_iter().map(
                |(size, groups, buttons_group, active_length)| {
                    let count = (buttons_group * groups) as f64;
                    let score = (d_size / size as f64
                        + d_groups / groups as f64
                        + d_buttons_group / buttons_group as f64
                        + d_active_length / active_length as f64)
                        * (d_count / count);

                    (
                        score.round() as usize,
                        (size, groups, buttons_group, active_length),
                    )
                },
            ));

        let mid = candidates.len().saturating_sub(1) / 2;

        let (button_size, groups, buttons_group, active_length) =
            candidates.into_iter().nth(mid).map_or(
                (max_button_size as f64, min_groups, min_buttons, safe_length),
                |(_, (button_size, groups, buttons_group, active_length))| {
                    (
                        button_size as f64,
                        groups,
                        buttons_group,
                        active_length as f64,
                    )
                },
            );

        let whitespace = (safe_length - active_length) / 2.0;
        let safe_area = if portrait {
            [
                safe_area[0],
                safe_area[1] + whitespace,
                safe_area[2],
                safe_area[3] + whitespace,
            ]
        } else {
            [
                safe_area[0] + whitespace,
                safe_area[1],
                safe_area[2] + whitespace,
                safe_area[3],
            ]
        };

        Config {
            portrait,
            width,
            height,
            length: active_length,
            breadth: safe_breadth,
            button_size,
            groups,
            buttons_group,
            button_track_margin: BUTTON_TRACK_MARGIN_RATION,
            safe_area,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RAND_SCREENS: &[(f64, f64, f64)] = &[
        (1920.0, 1080.0, 96.0),
        (2560.0, 1440.0, 110.0),
        (1366.0, 768.0, 125.0),
        (2732.0, 2048.0, 264.0),
        (2436.0, 1125.0, 458.0),
        (2960.0, 1440.0, 568.0),
        (3840.0, 2160.0, 163.0),
        (1280.0, 800.0, 180.0),
        (3440.0, 1440.0, 110.0),
        (2560.0, 1600.0, 227.0),
        (1080.0, 2340.0, 394.0),
        (6016.0, 3384.0, 220.0),
        (2048.0, 1536.0, 264.0),
        (2960.0, 1440.0, 522.0),
        (1280.0, 720.0, 267.0),
        (2560.0, 1440.0, 163.0),
        (1280.0, 1024.0, 96.0),
        (3840.0, 1080.0, 110.0),
        (2224.0, 1668.0, 264.0),
        (2960.0, 1440.0, 570.0),
        (3840.0, 1600.0, 163.0),
        (1280.0, 720.0, 326.0),
        (1920.0, 1200.0, 224.0),
        (2560.0, 1440.0, 141.0),
        (1366.0, 768.0, 100.0),
        (2560.0, 1440.0, 440.0),
        (1280.0, 800.0, 149.0),
        (2960.0, 1440.0, 522.0),
        (3840.0, 2160.0, 204.0),
        (2560.0, 1600.0, 197.0),
    ];

    #[test]
    fn config_snapshot_by_rand_screen() {
        for (i, config) in RAND_SCREENS
            .iter()
            .map(|(width, height, dpi)| Config::new(*width, *height, *dpi, [50.0, 20.0, 10.0, 25.0]))
            .enumerate()
        {
            insta::assert_yaml_snapshot!(format!("size: {i}"), config)
        }
    }
}
