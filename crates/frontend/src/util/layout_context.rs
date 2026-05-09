use common::{instrument::BandChannel, orientation::LayoutOrientation, safe_area::SafeArea, Line};
use leptos::prelude::*;
use mint::Vector2;

use super::tauri_resource::{use_tauri_resource, UseTauriResourceReturn};

pub fn provide_layout_context() {
    let UseTauriResourceReturn { data: layout, .. } =
        use_tauri_resource::<common::instrument::Layout>(common::instrument::events::LAYOUT);

    let space = Memo::new_owning(move |old| {
        let layout = layout.get().unwrap_or_default();
        let prop = layout.space;
        let has_changed = Some(prop) != old;
        (prop, has_changed)
    });
    let orientation = Memo::new_owning(move |old| {
        let layout = layout.get().unwrap_or_default();
        let prop = layout.orientation;
        let has_changed = Some(prop) != old;
        (prop, has_changed)
    });
    let left_string_position = Memo::new_owning(move |old| {
        let layout = layout.get().unwrap_or_default();
        let prop = layout.left_string_position;
        let has_changed = Some(prop) != old;
        (prop, has_changed)
    });
    let right_string_position = Memo::new_owning(move |old| {
        let layout = layout.get().unwrap_or_default();
        let prop = layout.right_string_position;
        let has_changed = Some(prop) != old;
        (prop, has_changed)
    });
    let key_radius = Memo::new_owning(move |old| {
        let layout = layout.get().unwrap_or_default();
        let prop = layout.key_radius;
        let has_changed = Some(prop) != old;
        (prop, has_changed)
    });
    let key_band_length = Memo::new_owning(move |old| {
        let layout = layout.get().unwrap_or_default();
        let prop = layout.key_band_length;
        let has_changed = Some(prop) != old;
        (prop, has_changed)
    });
    let key_band_breadth = Memo::new_owning(move |old| {
        let layout = layout.get().unwrap_or_default();
        let prop = layout.key_band_breadth;
        let has_changed = Some(prop) != old;
        (prop, has_changed)
    });
    let safe_area_padding = Memo::new_owning(move |old| {
        let layout = layout.get().unwrap_or_default();
        let prop = layout.safe_area_padding;
        let has_changed = Some(prop) != old;
        (prop, has_changed)
    });
    let key_bands_gap = Memo::new_owning(move |old| {
        let layout = layout.get().unwrap_or_default();
        let prop = layout.key_bands_gap;
        let has_changed = Some(prop) != old;
        (prop, has_changed)
    });
    let bands_gap = Memo::new_owning(move |old| {
        let layout = layout.get().unwrap_or_default();
        let prop = layout.bands_gap;
        let has_changed = Some(prop) != old;
        (prop, has_changed)
    });
    let num_keys_per_band = Memo::new_owning(move |old| {
        let layout = layout.get().unwrap_or_default();
        let prop = layout.num_keys_per_band.get();
        let has_changed = Some(prop) != old;
        (prop, has_changed)
    });
    let num_bands = Memo::new_owning(move |old| {
        let layout = layout.get().unwrap_or_default();
        let prop = layout.num_bands.get();
        let has_changed = Some(prop) != old;
        (prop, has_changed)
    });
    let first_band_channel = Memo::new_owning(move |old| {
        let layout = layout.get().unwrap_or_default();
        let prop = layout.first_band_channel;
        let has_changed = Some(prop) != old;
        (prop, has_changed)
    });
    let key_pad_main = Memo::new_owning(move |old| {
        let layout = layout.get().unwrap_or_default();
        let prop = layout.key_pad_main();
        let changed = Some(prop) != old;
        (prop, changed)
    });

    let fallback_scale = || {
        if let Some(window) = web_sys::window() {
            if let Some(document) = window.document() {
                // Detect dark mode by presence of any element with the 'dark' class,
                // as applied by the App wrapper (window_appearance_class).
                return document.query_selector(".dark").ok().flatten().is_some();
            }
        }
        false
    };

    let scale = Memo::new_owning(move |old| {
        if let Some(layout) = layout.get() {
            let prop = layout.scale;
            let has_changed = Some(prop) != old;
            (prop, has_changed)
        } else {
            match fallback_scale() {
                true => (common::instrument::Scale::In, true),
                false => (common::instrument::Scale::Yo, true),
            }
        }
    });

    let complete_layout = Memo::new_owning(move |old| {
        let layout = layout.get().unwrap_or_default();
        let has_changed = Some(layout) != old;
        (layout, has_changed)
    });

    provide_context(LayoutContextReturn {
        space,
        orientation,
        left_string_position,
        right_string_position,
        key_radius,
        key_band_length,
        key_band_breadth,
        safe_area_padding,
        key_bands_gap,
        bands_gap,
        num_keys_per_band,
        num_bands,
        first_band_channel,
        complete_layout,
        key_pad_main,
        scale,
    });
}

#[derive(Clone, Copy)]
pub struct LayoutContextReturn {
    pub space: Memo<Vector2<f64>>,
    pub orientation: Memo<LayoutOrientation>,
    pub left_string_position: Memo<Line>,
    pub right_string_position: Memo<Line>,
    pub key_radius: Memo<f64>,
    pub key_band_length: Memo<f64>,
    pub key_band_breadth: Memo<f64>,
    pub safe_area_padding: Memo<SafeArea>,
    pub key_bands_gap: Memo<f64>,
    pub bands_gap: Memo<f64>,
    pub num_keys_per_band: Memo<u8>,
    pub num_bands: Memo<u8>,
    pub first_band_channel: Memo<BandChannel>,
    pub complete_layout: Memo<common::instrument::Layout>,
    pub key_pad_main: Memo<f64>,
    pub scale: Memo<common::instrument::Scale>,
}

pub fn expect_layout_context() -> LayoutContextReturn {
    expect_context()
}
