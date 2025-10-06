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
    strings_group_rects: RwSignal<BTreeMap<usize, Bounding>>,
    strings_rect: RwSignal<Option<Bounding>>,

    animated_keys: RwSignal<HashSet<(usize, usize)>>,
    animated_bands: RwSignal<HashSet<(usize, usize)>>,
    animated_string_groups: RwSignal<HashSet<usize>>,
}

impl InstrumentContext {
    pub fn new() -> Self {
        Self {
            key_bboxes: RwSignal::new(BTreeMap::new()),
            band_bboxes: RwSignal::new(BTreeMap::new()),
            strings_group_rects: RwSignal::new(BTreeMap::new()),
            strings_rect: RwSignal::new(None),

            animated_keys: RwSignal::new(HashSet::new()),
            animated_bands: RwSignal::new(HashSet::new()),
            animated_string_groups: RwSignal::new(HashSet::new()),
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

    pub fn key_bbox(&self, id: (usize, usize)) -> Option<Bounding> {
        self.key_bboxes.get_untracked().get(&id).copied()
    }

    pub fn all_key_bboxes(&self) -> BTreeMap<(usize, usize), Bounding> {
        self.key_bboxes.get_untracked()
    }

    pub fn has_animated_key(&self, id: (usize, usize)) -> bool {
        self.animated_keys.get_untracked().contains(&id)
    }

    // --- Bands ---

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

    pub fn band_bbox(&self, id: (usize, usize)) -> Option<Bounding> {
        self.band_bboxes.get_untracked().get(&id).copied()
    }

    pub fn all_band_bboxes(&self) -> BTreeMap<(usize, usize), Bounding> {
        self.band_bboxes.get_untracked()
    }

    pub fn has_animated_band(&self, id: (usize, usize)) -> bool {
        self.animated_bands.get_untracked().contains(&id)
    }

    // --- Strings (per group) ---

    /// Insert or update a strings group's bounding box.
    /// Returns true only the first time this group is seen (to run group-level appear animation).
    pub fn upsert_strings_group_rect(&self, group: usize, bbox: Bounding) -> bool {
        self.strings_group_rects.update(|m| {
            m.insert(group, bbox);
        });

        let mut first_time = false;
        self.animated_string_groups.update(|s| {
            if !s.contains(&group) && bbox.is_non_empty() {
                s.insert(group);
                first_time = true;
            }
        });
        first_time
    }

    pub fn strings_group_rect(&self, group: usize) -> Option<Bounding> {
        self.strings_group_rects
            .get_untracked()
            .get(&group)
            .copied()
    }

    pub fn all_strings_group_rects(&self) -> BTreeMap<usize, Bounding> {
        self.strings_group_rects.get_untracked()
    }

    pub fn has_animated_strings_group(&self, group: usize) -> bool {
        self.animated_string_groups.get_untracked().contains(&group)
    }

    // --- Strings (overall) ---

    /// Set the overall strings rectangle (e.g., SVG viewport measured box).
    /// Returns true if this is the first time it becomes Some(..).
    pub fn set_strings_rect(&self, bbox: Bounding) -> bool {
        let was_none = self.strings_rect.get_untracked().is_none();
        self.strings_rect.set(Some(bbox));
        was_none && bbox.is_non_empty()
    }

    pub fn strings_rect(&self) -> Option<Bounding> {
        self.strings_rect.get_untracked()
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
