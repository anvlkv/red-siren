use std::str::FromStr;

mod button;
mod card;
mod content_page;
mod error_template;
mod icon;
mod instrument;
mod intro;
mod menu;
mod switch;
mod tooltip;

pub use button::*;
pub use card::*;
pub use content_page::*;
pub use error_template::*;
pub use icon::*;
pub use instrument::*;
pub use intro::*;
pub use menu::*;
pub use switch::*;
pub use tooltip::*;

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
            Self::Md => "p-4",
            Self::Lg => "p-8",
        }
    }
}
