use crate::{geometry::{Rect, Line}, instrument::Config};
use hecs::{Bundle, Entity, World};
use serde::{Deserialize, Serialize};

#[derive(Bundle, Clone, Copy, Serialize, Deserialize, PartialEq, Debug)]
pub struct Pair {
    pub value: Option<f32>,
    pub f_n: usize,
    pub rect: Rect,
}

impl Eq for Pair {}

impl Pair {
    fn new(f_n: usize, config: &Config, value: Option<f32>) -> Self {
        let pair_space_x = config.width / config.n_buttons as f64;
        let pair_mid_y = config.height / 2.0;

        let pair_rect_size = if config.button_size * 2.0 > pair_space_x {
            log::debug!("resize button for tuning");
            pair_space_x / 2.0
        } else {
            config.button_size
        };

        let x = pair_space_x * (f_n - 1) as f64 + (pair_space_x - pair_rect_size) / 2.0;
        let y = (pair_mid_y - pair_rect_size / 2.0) - value.clone().unwrap_or_default() as f64 * pair_mid_y;
        let rect = Rect::size(pair_rect_size, pair_rect_size)
            .offset_left_and_right(-x, x)
            .offset_top_and_bottom(-y, y);

        Pair {
            value,
            f_n,
            rect,
        }
    }

    pub fn spawn(world: &mut World, config: &Config, f_n: usize) -> Entity {
        world.spawn((Self::new(f_n, config, None),))
    }

    pub fn update_from_values(&mut self, values: &[(usize, f32)], config: &Config) {
        if let Some((_, value)) = values.iter().find(|(f, _)| f == &self.f_n) {
          self.set_value(*value, config)
        }
    }

    pub fn set_value(&mut self, value: f32, config: &Config)  {
      let next = Self::new(self.f_n, config, Some(value));
      *self = Self{
        ..next
      }
    }
}

#[derive(Bundle)]
pub struct Chart {
    pub pairs: Vec<Entity>,
    pub fft_values: Vec<(f32, f32)>,
    pub line: Line,
}

impl Chart {
    pub fn spawn(world: &mut World, config: &Config) -> Entity {
        let mut pairs = vec![];
        for i in 1..=config.n_buttons {
            pairs.push(Pair::spawn(world, config, i));
        }
        let mid_y = config.height / 2.0;
        let line = Line::new(0.0, config.width, mid_y, mid_y);
        world.spawn((Chart {
            pairs,
            fft_values: Default::default(),
            line
        },))
    }

    pub fn set_fft_data(&mut self, data: Vec<(f32, f32)>) {
        self.fft_values = data;
    }
}
