use leptos::prelude::*;
use std::collections::{BTreeMap, HashSet};

/// Instrument measurement context (MAYA DRY KISS)
///
/// Why:
/// - Centralized tracking of bounding boxes for keys, bands, and strings
/// - One-shot animation flags to avoid re-triggering appear animations on layout changes
/// - Strings animate per group; keep per-group rectangles and first-appearance flags
///
/// What:
/// - `key_bboxes[(g,k)]` and `band_bboxes[(g,k)]` hold measured boxes
/// - `strings_group_rects[g]` holds group-level boxes; `strings_rect` is the overall strings rect (optional)
/// - `animated_keys`, `animated_bands`, `animated_string_groups` are one-shot guards for appear animations
///
/// How:
/// - Call `upsert_*` when a node's bounding box becomes valid (> 0 size). The method returns `true` only for the first time,
///   which is your cue to attach appear (and optionally settle) animation classes.
/// - Subsequent size/position updates won't re-trigger first-appearance (return `false`).
#[derive(Clone)]
pub struct InstrumentContext {
    key_bboxes: RwSignal<BTreeMap<(usize, usize), Bounding>>,
    band_bboxes: RwSignal<BTreeMap<(usize, usize), Bounding>>,
    animated_keys: RwSignal<HashSet<(usize, usize)>>,
    animated_bands: RwSignal<HashSet<(usize, usize)>>,
}

impl InstrumentContext {
    pub fn new() -> Self {
        Self {
            key_bboxes: RwSignal::new(BTreeMap::new()),
            band_bboxes: RwSignal::new(BTreeMap::new()),
            animated_keys: RwSignal::new(HashSet::new()),
            animated_bands: RwSignal::new(HashSet::new()),
        }
    }

    // --- Keys ---

    /// Insert or update a key's bounding box.
    /// Returns true if this is the first time (suitable to run appear animation once).
    pub fn upsert_key_bbox(&self, id: (usize, usize), bbox: Bounding) -> bool {
        self.key_bboxes.update(|m| {
            m.insert(id, bbox);
        });

        // One-shot guard
        let mut first_time = false;
        self.animated_keys.update(|s| {
            if !s.contains(&id) && bbox.is_non_empty() {
                s.insert(id);
                first_time = true;
            }
        });
        first_time
    }

    /// Insert or update a band's bounding box.
    /// Returns true if this is the first time (suitable to run appear animation once).
    pub fn upsert_band_bbox(&self, id: (usize, usize), bbox: Bounding) -> bool {
        self.band_bboxes.update(|m| {
            m.insert(id, bbox);
        });

        // One-shot guard
        let mut first_time = false;
        self.animated_bands.update(|s| {
            if !s.contains(&id) && bbox.is_non_empty() {
                s.insert(id);
                first_time = true;
            }
        });
        first_time
    }
}

/// Bounding rectangle in CSS pixels.
///
/// Kept intentionally simple; callers provide values derived from leptos_use::use_element_bounding.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounding {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl Bounding {
    /// True if the rect has positive area.
    pub fn is_non_empty(&self) -> bool {
        self.width > 0.0 && self.height > 0.0
    }
}

/// Provide the instrument measurement context.
pub fn provide_instrument_context() {
    provide_context(InstrumentContext::new());
}

/// Expect the instrument measurement context.
///
/// Panics if not provided; call `provide_instrument_context()` in an ancestor first.
pub fn expect_instrument_context() -> InstrumentContext {
    expect_context()
}
