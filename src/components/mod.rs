use std::str::FromStr;

mod button;
mod card;
mod error_template;
mod icon;
mod intro;
mod menu;
mod page;
mod tooltip;
mod wavering;

pub use button::*;
pub use card::*;
pub use error_template::*;
pub use icon::*;
pub use intro::*;
pub use menu::*;
pub use page::*;
pub use tooltip::*;
pub use wavering::*;

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, strum::EnumString)]
pub enum UiVariant {
    #[default]
    Solid,
    Outline,
    Ghost,
}

impl From<String> for UiVariant {
    fn from(value: String) -> Self {
        Self::from_str(&value).expect("invalid button variant")
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
        Self::from_str(&value).expect("invalid button size")
    }
}
