use serde::{Deserialize, Serialize};

use crux_core::render::Render;
use crux_core::App;
use crux_kv::KeyValue;
use crux_macros::Effect;

#[derive(Default)]
pub struct Instrument;

#[derive(Default, Serialize, Deserialize)]
pub struct Model {
    pub config: Config,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone, Hash, PartialEq, Eq)]
pub struct Config {
    pub portrait: bool,
    pub width: u16,
    pub height: u16,
    pub groups: u8,
    pub buttons_group: u8,
    pub button_size: u8,
    pub gap: (u8, u8),
    pub steps: u16,
    pub breadth: u16,
    pub length: u16,
}

impl Config {
    pub fn new(
        width: usize,
        height: usize,
        rigid_step: bool,
        max_buttons: usize,
    ) -> Self {
        let portrait = height > width;
        let (length, breadth) = if portrait {
            (height, width)
        } else {
            (width, height)
        };

        let bs_ratio = breadth as f64 / 430.0;
        let button_size = bs_ratio * 78.0;
        let button_size = button_size.round() as usize;

        let mut button_gap = button_size / 3;
        let mut button_group_gap = button_gap * 2;

        let steps = if rigid_step {
            breadth / 2 / button_size
        } else {
            breadth / 2
        };


        let active_length = length - button_group_gap * 2;

        let groups: Vec<(Vec<(usize, usize)>, usize)> = (1..=active_length)
            .into_iter()
            .fold(Vec::<(Vec<(usize, usize)>, usize)>::new(), |mut acc, _| {
                if acc.last().is_some() {
                    let pos = acc.len() - 1;
                    let group = &mut acc[pos];
                    let group_len = group.0.len();

                    let mut last = group.0.last_mut();
                    let btn = last.as_mut().unwrap();
                    if btn.0 < button_size {
                        btn.0 += 1;
                    } else if btn.1 < button_gap && group_len < max_buttons {
                        btn.1 += 1;
                    } else if group_len < max_buttons {
                        group.0.push((1, 0));
                    } else if group.1 < button_group_gap {
                        group.1 += 1;
                    } else {
                        acc.push((vec![(1, 0)], 0));
                    }
                } else {
                    acc.push((vec![(1, 0)], 0));
                }
                acc
            })
            .into_iter()
            .map(|group| {
                (
                    group
                        .0
                        .into_iter()
                        .filter(|btn| btn.0 == button_size)
                        .collect(),
                    group.1,
                )
            })
            .collect();

        let optimal_buttons = groups.iter().map(|g| g.0.len()).max().unwrap_or(1);
        let optimal_groups = groups
            .iter()
            .filter(|g| g.0.len() == optimal_buttons)
            .count();

        if optimal_groups < groups.len() {
            let empty = groups.iter().skip(optimal_groups).fold(0, |acc, group| acc + group.1 + group.0.iter().fold(0, |a, b| a+b.0+b.1));
            let extra = empty / (optimal_groups * optimal_buttons);

            button_gap = button_gap + extra / optimal_groups;
            button_group_gap = button_gap + button_size;
        }

        Config {
            button_size: button_size.try_into().unwrap_or(42),
            portrait,
            width: width.try_into().unwrap_or(200),
            height: height.try_into().unwrap_or(200),
            steps: steps.try_into().unwrap_or(2),
            gap: (
                button_gap.try_into().unwrap_or(24),
                button_group_gap.try_into().unwrap_or(66),
            ),
            groups: optimal_groups.try_into().unwrap_or(1),
            buttons_group: optimal_buttons.try_into().unwrap_or(1),
            length: length.try_into().unwrap_or(200),
            breadth: (breadth / 3).try_into().unwrap_or(66)
        }
    }
}

#[derive(Default, Serialize, Deserialize, Clone, Hash, PartialEq, Eq)]
pub struct ViewModel {
    pub config: Config,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum Event {
    None,
}

#[cfg_attr(feature = "typegen", derive(crux_macros::Export))]
#[derive(Effect)]
#[effect(app = "Instrument")]
pub struct InstrumentCapabilities {
    pub render: Render<Event>,
    pub key_value: KeyValue<Event>,
}

impl App for Instrument {
    type Event = Event;

    type Model = Model;

    type ViewModel = ViewModel;

    type Capabilities = InstrumentCapabilities;

    fn update(&self, event: Self::Event, model: &mut Self::Model, caps: &Self::Capabilities) {
        todo!()
    }

    fn view(&self, model: &Self::Model) -> Self::ViewModel {
        ViewModel::default()
    }
}
