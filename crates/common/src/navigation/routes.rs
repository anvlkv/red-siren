use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};

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
#[strum(serialize_all = "kebab-case")]
pub enum EditorRouteId {
    Layout,
    FinetunedValues,
}

impl EditorRouteId {
    pub const fn path(self) -> &'static str {
        match self {
            Self::Layout => "/edit/layout",
            Self::FinetunedValues => "/edit/finetuned-values",
        }
    }

    pub const fn title(self) -> &'static str {
        match self {
            Self::Layout => "Layout editor",
            Self::FinetunedValues => "Fine tuning",
        }
    }
}

/// Typed identifiers for application routes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RouteId {
    Home,
    About,
    Donate,
    Play,
    Tune,
    Edit(EditorRouteId),
    TestNode,
    Permissions,
}

impl RouteId {
    pub const fn is_content(&self) -> bool {
        !matches!(self, Self::Play | Self::Tune)
    }

    pub const fn title(&self) -> &'static str {
        match self {
            Self::Home => "Home",
            Self::About => "About",
            Self::Donate => "Donate",
            Self::Play => "Play",
            Self::Tune => "Tune",
            Self::Edit(route) => route.title(),
            Self::TestNode => "Test Node",
            Self::Permissions => "Permissions",
        }
    }

    pub const fn path(&self) -> &'static str {
        match self {
            Self::Home => "/",
            Self::About => "/about",
            Self::Donate => "/donate",
            Self::Play => "/play",
            Self::Tune => "/tune",
            Self::Edit(route) => route.path(),
            Self::TestNode => "/test-node",
            Self::Permissions => "/permissions",
        }
    }
}

impl AsRef<str> for RouteId {
    fn as_ref(&self) -> &str {
        self.path()
    }
}

impl fmt::Display for RouteId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.path())
    }
}

impl FromStr for RouteId {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "/" => Ok(Self::Home),
            "/about" => Ok(Self::About),
            "/donate" => Ok(Self::Donate),
            "/play" => Ok(Self::Play),
            "/tune" => Ok(Self::Tune),
            "/edit" | "/edit/" | "/edit/layout" => Ok(Self::Edit(EditorRouteId::Layout)),
            "/edit/finetuned-values" => Ok(Self::Edit(EditorRouteId::FinetunedValues)),
            "/test-node" => Ok(Self::TestNode),
            "/permissions" => Ok(Self::Permissions),
            _ => Err(format!("unknown route: {value}")),
        }
    }
}
