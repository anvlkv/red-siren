use crate::{
    geometry::{Line, Rect},
    instrument::Config,
};
use hecs::{Bundle, Entity, World};
use mint::Point2;
use serde::{Deserialize, Serialize};

use super::{TuningValue, MAX_F, MIN_F};

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Debug, Default, PartialOrd, Ord)]
pub enum TriggerState {
    #[default]
    None,
    Ghost,
    Active,
}

#[derive(Bundle, Clone, Copy, Serialize, Deserialize, PartialEq, Debug)]
pub struct Pair {
    pub value: Option<(f32, f32)>,
    pub f_n: usize,
    pub rect: Rect,
    pub finger: Option<i32>,
    pub triggered: TriggerState,
}

impl Eq for Pair {}

impl Pair {
    fn new(f_n: usize, config: &Config, value: Option<(f32, f32)>) -> Self {
        let pair_space_x = config.width / config.n_buttons as f64;

        let pair_rect_size = if config.button_size * 2.0 > pair_space_x {
            log::debug!("resize button for tuning");
            pair_space_x / 2.0
        } else {
            config.button_size
        };

        let pair_min_y = config.height - config.safe_area[3];
        let x = config.width
            - (pair_space_x * (f_n - 1) as f64 + (pair_space_x - pair_rect_size) / 2.0);
        let y = (pair_min_y - pair_rect_size) - value.unwrap_or((0.0, 0.0)).0 as f64 * pair_min_y;
        let rect = Rect::size(pair_rect_size, pair_rect_size)
            .offset_left_and_right(-x, x)
            .offset_top_and_bottom(-y, y);

        Pair {
            value,
            f_n,
            rect,
            finger: None,
            triggered: TriggerState::default(),
        }
    }

    pub fn spawn(world: &mut World, config: &Config, f_n: usize) -> Entity {
        world.spawn((Self::new(f_n, config, None),))
    }
}

#[derive(Bundle)]
pub struct FFTChartEntry {
    pub pt_max: (Point2<f64>, Point2<f64>),
    pub amp_max: (f32, f32),
    pub freq: f32,
}

impl FFTChartEntry {
    fn spawn(
        world: &mut World,
        i: usize,
        config: &Config,
        total: usize,
        freq: f32,
        value: f32,
    ) -> Entity {
        let x = config.width - (config.width / total as f64) * i as f64;
        let pt = Self::value_point(x, config, value);
        world.spawn((Self {
            pt_max: (pt, pt),
            amp_max: (value, value),
            freq,
        },))
    }

    fn v_max(config: &Config) -> f64 {
        config.height - config.safe_area[1] - config.safe_area[3]
    }

    fn value_point(x: f64, config: &Config, value: f32) -> Point2<f64> {
        let v_max = Self::v_max(config);
        Point2 {
            x,
            y: config.height - (config.safe_area[3] + v_max * value as f64),
        }
    }

    fn point_amp(y: f64, config: &Config) -> f32 {
        let v_max = Self::v_max(config);
        (1.0 - ((y - config.safe_area[1]) / v_max)) as f32
    }

    fn apply_data(&mut self, freq: f32, value: f32, config: &Config) {
        self.freq = freq;
        self.amp_max.0 = value;
        self.amp_max.1 = self.amp_max.1.max(value);

        let x = self.pt_max.0.x;
        self.pt_max.0 = Self::value_point(x, config, self.amp_max.0);
        self.pt_max.1 = Self::value_point(x, config, self.amp_max.1);
    }
}

#[derive(Clone)]
pub struct Chart {
    pub pairs: Vec<Entity>,
    pub fft_values: Vec<Entity>,
    pub line: Line,
    pub scale: f64,
}

impl Chart {
    pub fn new(world: &mut World, config: &Config) -> Self {
        let mut pairs = vec![];
        for i in 1..=config.n_buttons {
            pairs.push(Pair::spawn(world, config, i));
        }
        pairs.reverse();
        let min_y = config.height - config.safe_area[3];
        let line = Line::new(0.0, config.width, min_y, min_y);
        Chart {
            pairs,
            fft_values: Default::default(),
            line,
            scale: 1.0,
        }
    }

    pub fn delete(self, world: &mut World) {
        for e in self.pairs {
            world.despawn(e).expect("delete pair");
        }

        for e in self.fft_values {
            world.despawn(e).expect("delete fft");
        }
    }

    pub fn set_fft_data(&mut self, world: &mut World, data: Vec<(f32, f32)>, config: &Config) {
        let total = data.len();
        for (i, (freq, value)) in data.clone().into_iter().enumerate() {
            if let Some(e) = self.fft_values.get(i) {
                let mut entry = world.get::<&mut FFTChartEntry>(*e).expect("entry");
                entry.apply_data(freq, value, config);
            } else {
                let e = FFTChartEntry::spawn(world, i, config, total, freq, value);
                self.fft_values.push(e);
            };
        }

        for p in self.pairs.iter() {
            let mut pair = world.get::<&mut Pair>(*p).expect("pair");

            pair.triggered = self
                .fft_values
                .iter()
                .map(|e| world.get::<&FFTChartEntry>(*e).ok())
                .flatten()
                .map(|entry| {
                    if pair.rect.contains(entry.pt_max.0) {
                        TriggerState::Active
                    } else if pair.rect.contains(entry.pt_max.1) {
                        TriggerState::Ghost
                    } else {
                        TriggerState::None
                    }
                })
                .fold(
                    TriggerState::None,
                    |acc, val| {
                        if acc > val {
                            acc
                        } else {
                            val
                        }
                    },
                );
        }
    }

    pub fn update_pairs_from_values(
        &self,
        world: &mut World,
        values: &[TuningValue],
        config: &Config,
    ) {
        let range_width = MAX_F - MIN_F;
        for (f_n, value_freq, value_amp) in values {
            let x = config.width - ((*value_freq - MIN_F) / range_width) as f64 * config.width;
            let pt = FFTChartEntry::value_point(x, config, *value_amp);
            if let Some((_, pair)) = world
                .query_mut::<&mut Pair>()
                .into_iter()
                .find(|(_, p)| p.f_n == *f_n)
            {
                pair.rect.move_x(pt.x);
                pair.rect.move_y(pt.y);
                pair.value = Some((*value_freq, *value_amp));
                log::info!("set pair {f_n} to {pt:?}");
            } else {
                log::warn!("no pair for fn {f_n}");
            }
        }
    }

    pub fn update_value_from_pos(
        &self,
        world: &mut World,
        f_n: usize,
        (x, y): (&f64, &f64),
        config: &Config,
    ) {
        let range_width = MAX_F - MIN_F;
        let value_freq = MAX_F - (x / config.width) as f32 * range_width + MIN_F;
        let value_amp = FFTChartEntry::point_amp(*y, config);
        let l_rect = if f_n > 1 {
            world
                .query::<&Pair>()
                .into_iter()
                .find(|(_, p)| p.f_n == f_n - 1)
                .map(|(_, p)| p.rect.clone())
        } else {
            None
        };

        let h_rect = world
            .query::<&Pair>()
            .into_iter()
            .find(|(_, p)| p.f_n == f_n + 1)
            .map(|(_, p)| p.rect.clone());

        let (_, pair) = world
            .query_mut::<&mut Pair>()
            .into_iter()
            .find(|(_, p)| p.f_n == f_n)
            .expect("pair for fn");

        if h_rect
            .map(|p| (p.center().x + config.button_size) < *x)
            .unwrap_or(true)
            && l_rect
                .map(|p| (p.center().x - config.button_size) > *x)
                .unwrap_or(true)
            && *y < config.height - config.safe_area[3]
            && *y > config.safe_area[1]
            && *x > config.safe_area[0]
            && *x < config.width - config.safe_area[2]
        {
            pair.value = Some((value_freq, value_amp));
            pair.rect.move_x(*x);
            pair.rect.move_y(*y);
        }
    }
}
