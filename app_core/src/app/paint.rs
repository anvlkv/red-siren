use std::hash::{Hash, Hasher};

pub use ecolor::Rgba;
use hecs::Bundle;
use keyframe::CanTween;
use serde::{Deserialize, Serialize};

const RED: Rgba = Rgba::from_rgb(227_f32, 0_f32, 34_f32);
const BLACK: Rgba = Rgba::from_rgb(53_f32, 56_f32, 57_f32);
const GRAY: Rgba = Rgba::from_rgb(54_f32, 69_f32, 79_f32);
const CINNABAR: Rgba = Rgba::from_rgb(228_f32, 77_f32, 46_f32);

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Debug, Hash)]
pub enum ObjectStyle {
    StringLine(usize),
    ChartLine,
    InstrumentButton,
    InstrumentTrack,
    MenuBkg,
    MenuButton,
}

impl ObjectStyle {
    pub fn fill(&self, dark: bool) -> Option<Rgba> {
        match self {
            Self::InstrumentButton | Self::MenuBkg => {
                if dark {
                    Some(RED)
                } else {
                    Some(BLACK)
                }
            }
            Self::InstrumentTrack | Self::MenuButton => {
                if dark {
                    Some(BLACK)
                } else {
                    Some(RED)
                }
            }
            _ => None,
        }
    }

    pub fn stroke(&self, dark: bool) -> Option<Stroke> {
        let color = if dark { RED } else { BLACK };
        match self {
            Self::StringLine(div) => Some(Stroke {
                color: color.multiply(1.0 / *div as f32),
                width: 1.0,
            }),
            Self::ChartLine | Self::InstrumentTrack => Some(Stroke { color, width: 1.0 }),
            _ => None,
        }
    }
}

#[derive(Bundle, Clone, Serialize, Deserialize, Hash)]
pub struct Paint {
    pub fill: Option<Rgba>,
    pub stroke: Option<Stroke>,
    pub style: ObjectStyle,
}

impl Paint {
    pub fn new(dark: bool, style: ObjectStyle) -> Self {
        Self {
            fill: style.fill(dark),
            stroke: style.stroke(dark),
            style,
        }
    }

    pub fn repaint(&mut self, dark: bool) {
        self.fill = self.style.fill(dark);
        self.stroke = self.style.stroke(dark);
    }
}

impl CanTween for Paint {
    fn ease(from: Self, to: Self, time: impl keyframe::num_traits::Float) -> Self {
        let fill = ease_color_option(from.fill, to.fill, time);
        let stroke = if from.stroke.is_none() && to.stroke.is_none() {
            None
        } else {
            Some(CanTween::ease(
                from.stroke
                    .clone()
                    .unwrap_or(Stroke::zero(to.stroke.as_ref().map(|s| s.color))),
                to.stroke
                    .clone()
                    .unwrap_or(Stroke::zero(from.stroke.as_ref().map(|s| s.color))),
                time,
            ))
        };
        assert_eq!(from.style, to.style);

        let style = from.style;

        Self {
            fill,
            stroke,
            style,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Stroke {
    pub color: Rgba,
    pub width: f64,
}

impl Hash for Stroke {
    fn hash<H>(&self, state: &mut H)
    where
        H: Hasher,
    {
        let bin = bincode::serialize(self).unwrap();
        bin.hash(state)
    }
}

impl Stroke {
    pub fn zero(color: Option<Rgba>) -> Self {
        Self {
            color: color.unwrap_or(Rgba::TRANSPARENT),
            width: 0.0,
        }
    }
}

impl CanTween for Stroke {
    fn ease(from: Self, to: Self, time: impl keyframe::num_traits::Float) -> Self {
        let width = CanTween::ease(from.width, to.width, time);
        let color = ease_color_option(Some(from.color), Some(to.color), time).unwrap();
        Self { width, color }
    }
}

fn ease_color_option(
    from: Option<Rgba>,
    to: Option<Rgba>,
    time: impl keyframe::num_traits::Float,
) -> Option<Rgba> {
    match (from, to) {
        (None, None) => None,
        (Some(c1), None) => {
            let a = CanTween::ease(c1.a(), 0.0, time);
            Some(Rgba::from_rgba_unmultiplied(c1.r(), c1.g(), c1.b(), a))
        }
        (None, Some(c1)) => {
            let a = CanTween::ease(0.0, c1.a(), time);
            Some(Rgba::from_rgba_unmultiplied(c1.r(), c1.g(), c1.b(), a))
        }
        (Some(c1), Some(c2)) => {
            let r = CanTween::ease(c1.r(), c2.r(), time);
            let g = CanTween::ease(c1.g(), c2.g(), time);
            let b = CanTween::ease(c1.b(), c2.b(), time);
            let a = CanTween::ease(c1.a(), c2.a(), time);
            Some(Rgba::from_rgba_unmultiplied(r, g, b, a))
        }
    }
}
