//! WHY:
//! Introduces a typed `RouteId` so frontend (Leptos) and backend (Tauri)
//! share a single source of truth for routes. This is Phase 1 of the
//! navigation rework enabling future orchestration (leave/enter hooks,
//! gating, cancellation, etc.).
//!
//! DESIGN NOTES:
//! - Flat enum (no nested segment awareness per current requirements).
//! - Provides path mapping + reverse lookup.
//! - Includes a tolerant parser (`parse_route_id`) to support an eager
//!   payload migration (Option B) with backward compatibility: it accepts:
//!     * Raw path strings ("/")
//!     * JSON string of the enum variant name:  "\"Home\""
//!     * Raw bare variant name: Home
//!     * JSON object with { "path": "<path>" } or { "route": "<Variant>" }
//! - Same-route navigation policy (ignore) will be enforced later by
//!   the navigation manager (Phase 2).
//!
//! FUTURE (Phase 2+):
//! - Real state machine will use `RouteId` everywhere.
//! - Backend gating (Tauri) will expose a `can_navigate(to: RouteId)`
//!   function / command. (Stub comment added in backend file separately.)
//!
//! KEEP SIMPLE (MAYA DRY KISS):
//! - No dynamic registration, just an enum + helpers.
//! - Extend by adding variants + arms; everything else auto-updates.
//!
//! To add a new route later:
//! 1. Add variant to `RouteId`.
//! 2. Add its path mapping in `path()` and reverse in `route_from_path()`.
//! 3. (Optional) Adjust any per-route metadata once introduced.

use core::fmt;
use serde::{Deserialize, Serialize};

/// Typed identifiers for application routes.
///
/// Extend by uncommenting / adding variants.
/// Keep ordering stable to avoid noisy diffs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RouteId {
    Home,
    About,
    Play,
    Permissions,
}

impl RouteId {
    /// Returns the canonical path for the route (leading slash, no trailing slash except root).
    pub const fn path(&self) -> &'static str {
        match self {
            RouteId::Home => "/",
            RouteId::About => "/about",
            RouteId::Play => "/play",
            RouteId::Permissions => "/permissions",
        }
    }

    /// All defined routes (stable slice for iteration / validation).
    pub const fn all() -> &'static [RouteId] {
        &[
            RouteId::Home,
            RouteId::About,
            RouteId::Play,
            RouteId::Permissions,
        ]
    }
}

/// Normalize a path string:
/// - Ensures leading slash
/// - Trims trailing slash (unless root)
fn normalize_path(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "/".to_string();
    }
    let with_slash = if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{}", trimmed)
    };
    if with_slash != "/" && with_slash.ends_with('/') {
        with_slash.trim_end_matches('/').to_string()
    } else {
        with_slash
    }
}

/// Reverse mapping: path -> RouteId
pub fn route_from_path(path: &str) -> Option<RouteId> {
    let p = normalize_path(path);
    match p.as_str() {
        "/" => Some(RouteId::Home),
        "/about" => Some(RouteId::About),
        "/play" => Some(RouteId::Play),
        "/permissions" => Some(RouteId::Permissions),
        _ => None,
    }
}

/// Backward-compatible parser for incoming navigation payloads.
///
/// Accepted forms (examples):
/// - "/"                      (raw path)
/// - "Home"                   (raw variant name)
/// - "\"Home\""               (JSON string of variant)
/// - "{\"path\":\"/\"}"       (JSON object with path)
/// - "{\"route\":\"Home\"}"   (JSON object with variant)
///
/// Returns `None` if unrecognized.
pub fn parse_route_id(input: &str) -> Option<RouteId> {
    let trimmed = input.trim();

    if trimmed.is_empty() {
        return None;
    }

    // 1. Try direct path mapping (most common legacy case).
    if let Some(r) = route_from_path(trimmed) {
        return Some(r);
    }

    // 2. Try JSON string (serde unit enum as a string).
    if (trimmed.starts_with('"') && trimmed.ends_with('"'))
        || (trimmed.chars().all(|c| c.is_ascii_alphanumeric()) && trimmed.len() <= 32)
    {
        // Attempt to interpret as variant name (strip quotes if present).
        let bare = trimmed.trim_matches('"').trim_matches('\'').trim();

        if let Some(r) = parse_variant_name(bare) {
            return Some(r);
        }
    }

    // 3. Try full JSON deserialization into RouteId directly.
    if let Ok(r) = serde_json::from_str::<RouteId>(trimmed) {
        return Some(r);
    }

    // 4. Try structured object: { "path": "..."} / { "route": "Variant" }
    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        // Light manual parse to avoid allocating full dynamic struct for trivial shapes.
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
            if let Some(r) = val
                .get("path")
                .and_then(|v| v.as_str())
                .and_then(route_from_path)
            {
                return Some(r);
            }
            if let Some(r) = val
                .get("route")
                .and_then(|v| v.as_str())
                .and_then(parse_variant_name)
            {
                return Some(r);
            }
        }
    }

    None
}

/// Interpret a raw variant name (case-insensitive) into a RouteId.
fn parse_variant_name(name: &str) -> Option<RouteId> {
    let lower = name.to_ascii_lowercase();
    match lower.as_str() {
        "home" => Some(RouteId::Home),
        "about" => Some(RouteId::About),
        "play" => Some(RouteId::Play),
        "permissions" => Some(RouteId::Permissions),
        _ => None,
    }
}

impl fmt::Display for RouteId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RouteId::Home => write!(f, "Home"),
            RouteId::About => write!(f, "About"),
            RouteId::Play => write!(f, "Play"),
            RouteId::Permissions => write!(f, "Permissions"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_mapping_round_trip() {
        for r in RouteId::all() {
            let p = r.path();
            let parsed = route_from_path(p).expect("reverse map");
            assert_eq!(&parsed, r);
        }
    }

    #[test]
    fn parse_variants_and_paths() {
        assert_eq!(parse_route_id("/"), Some(RouteId::Home));
        assert_eq!(parse_route_id(" / "), Some(RouteId::Home));
        assert_eq!(parse_route_id("\"Home\""), Some(RouteId::Home));
        assert_eq!(parse_route_id("Home"), Some(RouteId::Home));
        assert_eq!(parse_route_id("{\"route\":\"Home\"}"), Some(RouteId::Home));
        assert_eq!(parse_route_id("{\"path\":\"/\"}"), Some(RouteId::Home));
        assert_eq!(parse_route_id("Unknown"), None);
    }
}
