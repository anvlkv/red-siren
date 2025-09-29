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
    None,
    #[strum(serialize = "/home")]
    Home,
    #[strum(serialize = "/about")]
    About,
    #[strum(serialize = "/donate")]
    Donate,
    #[strum(serialize = "/play")]
    Play,
    #[strum(serialize = "/tune")]
    Tune,
    #[strum(serialize = "/permissions")]
    Permissions,
}
