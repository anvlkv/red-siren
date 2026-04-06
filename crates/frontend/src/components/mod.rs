use std::str::FromStr;

mod appearance_toggle;
mod button;
mod card;
mod content_page;
mod editor;
mod error_template;
mod excitement_source_toggle;
mod fold;
mod icon;
mod instrument;
mod intro;
mod menu;
mod notifications;
mod playback_quality_switch;
mod range_slider;
mod routed_tabs;
mod switch;
mod toaster;
mod tooltip;
mod tuner;

pub use appearance_toggle::*;
pub use button::*;
pub use card::*;
pub use content_page::*;
pub use editor::*;
pub use error_template::*;
pub use excitement_source_toggle::*;
pub use fold::*;
pub use icon::*;
pub use instrument::*;
pub use intro::*;
pub use menu::*;
pub use notifications::*;
pub use playback_quality_switch::*;
pub use range_slider::*;
pub use routed_tabs::*;
pub use switch::*;
pub use toaster::*;
pub use tooltip::*;
pub use tuner::*;

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, strum::EnumString)]
pub enum UiVariant {
    #[default]
    Solid,
    Outline,
    Ghost,
}

impl From<String> for UiVariant {
    fn from(value: String) -> Self {
        Self::from_str(&value).expect("invalid variant")
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, strum::EnumString)]
pub enum UiSize {
    Sm,
    Md,
    #[default]
    Lg,
}

impl From<String> for UiSize {
    fn from(value: String) -> Self {
        Self::from_str(&value).expect("invalid size")
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, strum::EnumString)]
#[allow(dead_code)]
pub enum UiPadding {
    None,
    Sm,
    Md,
    #[default]
    Lg,
}

impl From<String> for UiPadding {
    fn from(value: String) -> Self {
        Self::from_str(&value).expect("invalid padding")
    }
}

impl UiPadding {
    pub fn tw_class(&self) -> &'static str {
        match self {
            Self::None => "p-0",
            Self::Sm => "p-1",
            Self::Md => "md:p-4 p-2",
            Self::Lg => "md:p-8 p-3",
        }
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, strum::EnumString)]
pub enum UiPlacement {
    Top,
    #[default]
    Bottom,
    Left,
    Right,
}

impl UiPlacement {
    pub fn is_vertical(self) -> bool {
        matches!(self, UiPlacement::Left | UiPlacement::Right)
    }

    pub fn opposite(self) -> Self {
        match self {
            UiPlacement::Top => UiPlacement::Bottom,
            UiPlacement::Bottom => UiPlacement::Top,
            UiPlacement::Left => UiPlacement::Right,
            UiPlacement::Right => UiPlacement::Left,
        }
    }
}

impl From<String> for UiPlacement {
    fn from(value: String) -> Self {
        use std::str::FromStr;
        UiPlacement::from_str(&value).expect("invalid placement")
    }
}
