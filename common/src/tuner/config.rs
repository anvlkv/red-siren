use mint::{Point2, Vector2};

use crate::tuner::layout::Layout;
use crate::NodeKey;

/// Represents the full tuner data set (layout + sensors + FFT mapping).
#[derive(Debug, PartialEq, Clone, Default)]
pub struct Config {
    pub layout: Layout,
    pub sensor_data: Vec<SensorData>,
    pub fft_size: usize,
    pub sample_rate: f32,
}

#[derive(Debug, PartialEq, Clone, Copy, Default)]
pub struct SensorData {
    pub key: NodeKey,
    pub min_frequency: f32, // Hz
    pub max_frequency: f32, // Hz
    pub min_magnitude: f32, // Linear magnitude threshold
    pub max_magnitude: f32, // Linear magnitude threshold
}

impl Config {
    pub fn frequency_magnitude_to_space(
        &self,
        frequency: f32,
        magnitude: f32,
    ) -> mint::Point2<f32> {
        let Vector2 {
            x: space_width,
            y: space_height,
        } = self.layout.space;

        // Map frequency to 0-1 range based on sample rate
        let nyquist = self.sample_rate / 2.0;
        let freq_ratio = (frequency / nyquist).clamp(0.0, 1.0);

        // Magnitude is already normalized 0-1

        let (x, y) = match self.layout.orientation {
            crate::orientation::LayoutOrientation::Vertical => {
                let x = magnitude * space_width;
                let y = freq_ratio * space_height;
                (x, y)
            }
            crate::orientation::LayoutOrientation::Horizontal => {
                let x = freq_ratio * space_width;
                let y = magnitude * space_height;
                (x, y)
            }
        };

        Point2 { x, y }
    }

    pub fn space_to_frequency_magnitude(&self, point: mint::Point2<f32>) -> (f32, f32) {
        let Vector2 {
            x: space_width,
            y: space_height,
        } = self.layout.space;

        if space_width == 0.0 || space_height == 0.0 {
            return (0.0, 0.0);
        }

        let x = point.x;
        let y = point.y;

        let nyquist = self.sample_rate / 2.0;

        let (magnitude, freq_ratio) = match self.layout.orientation {
            crate::orientation::LayoutOrientation::Vertical => {
                let magnitude = (x / space_width).clamp(0.0, 1.0);
                let freq_ratio = (y / space_height).clamp(0.0, 1.0);
                (magnitude, freq_ratio)
            }
            crate::orientation::LayoutOrientation::Horizontal => {
                let freq_ratio = (x / space_width).clamp(0.0, 1.0);
                let magnitude = (y / space_height).clamp(0.0, 1.0);
                (magnitude, freq_ratio)
            }
        };

        let frequency = freq_ratio * nyquist;

        (frequency, magnitude)
    }
}
