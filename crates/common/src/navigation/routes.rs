use serde::{Deserialize, Serialize};

/// Typed identifiers for application routes.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    strum::EnumString,
    strum::IntoStaticStr,
    strum::Display,
    strum::VariantNames,
    strum::AsRefStr,
)]
#[strum(serialize_all = "snake_case")]
pub enum RouteId {
    #[strum(serialize = "/")]
    Home,
    #[strum(serialize = "/about")]
    About,
    #[strum(serialize = "/donate")]
    Donate,
    #[strum(serialize = "/play")]
    Play,
    #[strum(serialize = "/tune")]
    Tune,
    #[strum(serialize = "/edit")]
    Edit,
    #[strum(serialize = "/permissions")]
    Permissions,
}

impl RouteId {
    pub fn is_content(&self) -> bool {
        !matches!(self, Self::Play | Self::Tune)
    }

    pub fn title(&self) -> &'static str {
        match self {
            Self::Home => "Home",
            Self::About => "About",
            Self::Donate => "Donate",
            Self::Play => "Play",
            Self::Tune => "Tune",
            Self::Edit => "Edit",
            Self::Permissions => "Permissions",
        }
    }
}
