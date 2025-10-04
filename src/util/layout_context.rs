use common::{
    instrument::{GroupChanel, Scale},
    orientation::LayoutOrientation,
    safe_area::SafeArea,
    Line,
};
use leptos::prelude::*;
use mint::Vector2;

use super::tauri_resource::{use_tauri_resource, UseTauriResourceReturn};

#[derive(Debug, Clone, Copy)]
struct LayoutContext(Memo<common::instrument::Layout>);

pub fn provide_layout_context() {
    let UseTauriResourceReturn { data: layout, .. } =
        use_tauri_resource::<common::instrument::Layout>(common::instrument::events::LAYOUT);

    provide_context(LayoutContext(Memo::new(move |_| {
        layout().unwrap_or_default()
    })));
}

pub struct LayoutContextReturn {
    pub space: Memo<Vector2<f32>>,
    pub orientation: Memo<LayoutOrientation>,
    pub left_string_position: Memo<Line>,
    pub right_string_position: Memo<Line>,
    pub key_radius: Memo<f32>,
    pub key_band_length: Memo<f32>,
    pub key_band_breadth: Memo<f32>,
    pub safe_area_padding: Memo<SafeArea>,
    pub key_bands_gap: Memo<f32>,
    pub groups_gap: Memo<f32>,
    pub num_keys_per_group: Memo<u8>,
    pub num_groups: Memo<u8>,
    pub first_group_channel: Memo<GroupChanel>,
    pub scale: Memo<Scale>,
    pub complete_layout: Memo<common::instrument::Layout>,
    pub key_pad_main: Memo<f32>,
}

pub fn expect_layout_contex() -> LayoutContextReturn {
    let LayoutContext(layout) = expect_context();

    let space = Memo::new_owning(move |old| {
        let layout = layout.get();
        let prop = layout.space;
        let has_changed = Some(prop) != old;
        (prop, has_changed)
    });
    let orientation = Memo::new_owning(move |old| {
        let layout = layout.get();
        let prop = layout.orientation;
        let has_changed = Some(prop) != old;
        (prop, has_changed)
    });
    let left_string_position = Memo::new_owning(move |old| {
        let layout = layout.get();
        let prop = layout.left_string_position;
        let has_changed = Some(prop) != old;
        (prop, has_changed)
    });
    let right_string_position = Memo::new_owning(move |old| {
        let layout = layout.get();
        let prop = layout.right_string_position;
        let has_changed = Some(prop) != old;
        (prop, has_changed)
    });
    let key_radius = Memo::new_owning(move |old| {
        let layout = layout.get();
        let prop = layout.key_radius;
        let has_changed = Some(prop) != old;
        (prop, has_changed)
    });
    let key_band_length = Memo::new_owning(move |old| {
        let layout = layout.get();
        let prop = layout.key_band_length;
        let has_changed = Some(prop) != old;
        (prop, has_changed)
    });
    let key_band_breadth = Memo::new_owning(move |old| {
        let layout = layout.get();
        let prop = layout.key_band_breadth;
        let has_changed = Some(prop) != old;
        (prop, has_changed)
    });
    let safe_area_padding = Memo::new_owning(move |old| {
        let layout = layout.get();
        let prop = layout.safe_area_padding;
        let has_changed = Some(prop) != old;
        (prop, has_changed)
    });
    let key_bands_gap = Memo::new_owning(move |old| {
        let layout = layout.get();
        let prop = layout.key_bands_gap;
        let has_changed = Some(prop) != old;
        (prop, has_changed)
    });
    let groups_gap = Memo::new_owning(move |old| {
        let layout = layout.get();
        let prop = layout.groups_gap;
        let has_changed = Some(prop) != old;
        (prop, has_changed)
    });
    let num_keys_per_group = Memo::new_owning(move |old| {
        let layout = layout.get();
        let prop = layout.num_keys_per_group.get();
        let has_changed = Some(prop) != old;
        (prop, has_changed)
    });
    let num_groups = Memo::new_owning(move |old| {
        let layout = layout.get();
        let prop = layout.num_groups.get();
        let has_changed = Some(prop) != old;
        (prop, has_changed)
    });
    let first_group_channel = Memo::new_owning(move |old| {
        let layout = layout.get();
        let prop = layout.first_group_channel;
        let has_changed = Some(prop) != old;
        (prop, has_changed)
    });
    let scale = Memo::new_owning(move |old| {
        let layout = layout.get();
        let prop = layout.scale;
        let has_changed = Some(prop) != old;
        (prop, has_changed)
    });
    let key_pad_main = Memo::new_owning(move |old| {
        let l = layout.get();
        let prop = l.key_pad_main();
        let changed = Some(prop) != old;
        (prop, changed)
    });

    LayoutContextReturn {
        complete_layout: layout,
        space,
        orientation,
        left_string_position,
        right_string_position,
        key_radius,
        key_band_length,
        key_band_breadth,
        safe_area_padding,
        key_bands_gap,
        groups_gap,
        num_keys_per_group,
        num_groups,
        first_group_channel,
        scale,
        key_pad_main,
    }
}
