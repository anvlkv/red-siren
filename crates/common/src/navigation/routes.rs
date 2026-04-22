use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum EditorRouteId {
    #[default]
    InstrumentLayout,
    FinetunedValues,
    RhythmGrid,
    NodeTestBed,
}

impl EditorRouteId {
    pub const fn path(self) -> &'static str {
        match self {
            Self::InstrumentLayout => "/edit/layout",
            Self::FinetunedValues => "/edit/finetuned-values",
            Self::RhythmGrid => "/edit/rhythm-grid",
            Self::NodeTestBed => "/edit/node-test-bed",
        }
    }

    pub const fn title(self) -> &'static str {
        match self {
            Self::InstrumentLayout => "Layout editor",
            Self::FinetunedValues => "Fine tuning",
            Self::RhythmGrid => "Rhythm grid editor",
            Self::NodeTestBed => "Node test bed",
        }
    }
}

impl AsRef<str> for EditorRouteId {
    fn as_ref(&self) -> &str {
        match self {
            Self::InstrumentLayout => "/layout",
            Self::FinetunedValues => "/finetuned-values",
            Self::RhythmGrid => "/rhythm-grid",
            Self::NodeTestBed => "/node-test-bed",
        }
    }
}

impl FromStr for EditorRouteId {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "layout" => Ok(Self::InstrumentLayout),
            "finetuned-values" => Ok(Self::FinetunedValues),
            "rhythm-grid" => Ok(Self::RhythmGrid),
            "node-test-bed" => Ok(Self::NodeTestBed),
            _ => Err(format!("unknown route: {value}")),
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
            "/edit" => Ok(Self::Edit(EditorRouteId::default())),
            r if r.starts_with("/edit/") => {
                let suffix = &r["/edit/".len()..];
                let editor_route = EditorRouteId::from_str(suffix)?;
                Ok(Self::Edit(editor_route))
            }
            "/permissions" => Ok(Self::Permissions),
            _ => Err(format!("unknown route: {value}")),
        }
    }
}
