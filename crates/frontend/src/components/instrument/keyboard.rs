use common::safe_area::SafeArea;
use leptos::prelude::*;
use tauri_use::{use_command, UseTauriWithReturn};

use crate::util::layout_context::{expect_layout_contex, LayoutContextReturn};

use super::element::KeyboardElement;

const BAND_STROKE_WIDTH: f32 = 4.0;

#[component]
pub fn Keyboard() -> impl IntoView {
    let LayoutContextReturn {
        orientation,
        space,
        safe_area_padding,
        num_groups,
        key_band_length,
        groups_gap,
        key_bands_gap,
        key_band_breadth,
        key_radius,
        key_pad_main,
        ..
    } = expect_layout_contex();

    // Excitement snoop batch stream (pull model).
    let UseTauriWithReturn {
        trigger: fetch_batch,
        data: batch_data,
        error: batch_error,
        ..
    } = use_command::<common::instrument::data::ExcitementSnoopBatchPayload>(
        common::instrument::data::GET_ALL_ACTIVATION_SNOOPS,
    );

    // Log errors for the batch command
    Effect::new(move |_| {
        if let Some(err) = batch_error() {
            log::error!(
                "Error invoking {}: {err}",
                common::instrument::data::GET_ALL_ACTIVATION_SNOOPS
            );
        }
    });

    // Drive periodic fetch (20 FPS)
    let _raf = crate::util::raf_fn_fps::use_raf_fn_with_fps(
        move |_| {
            fetch_batch(Some(()));
        },
        20.0,
    );

    let main_container_axis_style = Memo::new(move |_| {
        let orientation = orientation();
        let space = space();
        let safe_area_padding = safe_area_padding();
        let num_groups = num_groups();
        let key_band_length = key_band_length();
        let groups_gap = groups_gap();
        let key_bands_gap = key_bands_gap();
        let key_band_breadth = key_band_breadth();
        let key_radius = key_radius();
        let pad_main = key_pad_main();

        // Map the orientation-dependent safe area format into CSS box-model (top,right,bottom,left)
        let SafeArea {
            top: safe_top,
            right: safe_right,
            bottom: safe_bottom,
            left: safe_left,
        } = safe_area_padding;

        let mut defs = match orientation {
            common::orientation::LayoutOrientation::Vertical => format!(
                r#"
                --keyboard-rows: repeat({rows}, minmax(0, 1fr));
                --keyboard-cols: repeat({cols}, minmax(0, 1fr));
                --keyboard-row-gap: {row_gap}px;
                --keyboard-col-gap: {col_gap}px;
                --keyboard-pad-left: {pad_left}px;
                --keyboard-pad-right: {pad_right}px;
                --keyboard-pad-top: {pad_top}px;
                --keyboard-pad-bottom: {pad_bottom}px;
                "#,
                rows = num_groups,
                cols = 1,
                row_gap = groups_gap,
                col_gap = 0,
                pad_left = safe_left,
                pad_right = safe_right,
                pad_top = pad_main.max(safe_top),
                pad_bottom = pad_main.max(safe_bottom)
            ),
            common::orientation::LayoutOrientation::Horizontal => format!(
                r#"
                --keyboard-rows: repeat({rows}, minmax(0, 1fr));
                --keyboard-cols: repeat({cols}, minmax(0, 1fr));
                --keyboard-row-gap: {row_gap}px;
                --keyboard-col-gap: {col_gap}px;
                --keyboard-pad-left: {pad_left}px;
                --keyboard-pad-right: {pad_right}px;
                --keyboard-pad-top: {pad_top}px;
                --keyboard-pad-bottom: {pad_bottom}px;
                "#,
                rows = 1,
                cols = num_groups,
                row_gap = 0,
                col_gap = groups_gap,
                pad_left = pad_main.max(safe_left),
                pad_right = pad_main.max(safe_right),
                pad_top = safe_top,
                pad_bottom = safe_bottom
            ),
        };

        defs.push_str(&format!("--keyboard-keys-gap: {}px;", key_bands_gap));

        // Compute padding so the key is exactly centered inside the band.
        let inner_band = key_band_breadth;
        let desired_diameter = key_radius * 2.0;

        defs.push_str(&format!(
            r#"
            --keyboard-band-breadth: {breadth}px;
            --keyboard-band-length: {length}px;
            --keyboard-key-diameter: {diameter}px;
            --keyboard-key-padding: {key_pad}px;
            --keyboard-band-stroke-width: {band_stroke}px;

            /* Explicit sizing + padding for outer container */
            width: {space_w}px;
            height: {space_h}px;
            padding: {safe_top}px {safe_right}px {safe_bottom}px {safe_left}px;
            "#,
            breadth = key_band_breadth,
            length = key_band_length,
            diameter = { desired_diameter.min(inner_band.max(0.0)) },
            key_pad = {
                // Clamp diameter to the available inner breadth (never negative)
                let dia = desired_diameter.min(inner_band.max(0.0));
                // Evenly distribute the remaining space on both sides to keep the key perfectly centered
                ((inner_band - dia) / 2.0).max(0.0)
            },
            space_w = space.x,
            space_h = space.y,
            safe_top = safe_top,
            safe_right = safe_right,
            safe_bottom = safe_bottom,
            safe_left = safe_left,
            band_stroke = BAND_STROKE_WIDTH,
        ));

        defs
    });

    view! {
        <div style=main_container_axis_style class="flex items-center justify-center">
            <div class="grid items-center justify-center grid-rows-(--keyboard-rows) grid-cols-(--keyboard-cols) gap-y-(--keyboard-row-gap) gap-x-(--keyboard-col-gap) p-t-(length:--keyboard-pad-top) p-b-(length:--keyboard-pad-bottom) p-l-(length:--keyboard-pad-left) p-r-(length:--keyboard-pad-right) w-full h-full">
                {move || {
                    let num_groups = num_groups();
                    (0..num_groups as usize)
                        .rev()
                        .map(|g| {
                            view! { <Group g batch_data=batch_data /> }
                        })
                        .collect_view()
                }}
            </div>
        </div>
    }
}

#[component]
fn Group(
    g: usize,
    #[prop(into)] batch_data: Signal<Option<common::instrument::data::ExcitementSnoopBatchPayload>>,
) -> impl IntoView {
    let LayoutContextReturn {
        orientation,
        num_keys_per_group,
        first_group_channel,
        key_band_length,
        key_band_breadth,
        ..
    } = expect_layout_contex();

    let class = move || {
        format!(
            "flex {} gap-(--keyboard-keys-gap) justify-center items-center",
            match orientation() {
                common::orientation::LayoutOrientation::Vertical => "flex-col w-full",
                common::orientation::LayoutOrientation::Horizontal => "flex-row h-full",
            }
        )
    };

    let group_band_dims_style = move || {
        let len = key_band_length();
        let br = key_band_breadth();
        let square_size = br.min(len);
        let (final_w, final_h) = match orientation() {
            common::orientation::LayoutOrientation::Vertical => (len, br),
            common::orientation::LayoutOrientation::Horizontal => (br, len),
        };
        format!(
            "--inst-band-k1-width: {}px; --inst-band-k1-height: {}px; --inst-band-final-width: {}px; --inst-band-final-height: {}px;",
            square_size, square_size, final_w, final_h
        )
    };
    view! {
        <div class=class style=group_band_dims_style>
            {move || {
                let first_group_channel = first_group_channel();
                let num_keys_per_group = num_keys_per_group();
                let orientation = orientation();
                (0..(num_keys_per_group as usize))
                    .rev()
                    .map(move |k| {
                        {
                            let samples = Signal::derive(move || {
                                batch_data()
                                    .and_then(|b| {
                                        b.snoops
                                            .iter()
                                            .find(|e| e.group as usize == g && e.key as usize == k)
                                            .map(|e| e.samples.clone())
                                    })
                            });
                            view! {
                                <KeyboardElement
                                    g
                                    k
                                    first_group_channel
                                    orientation
                                    excitement_samples=samples
                                />
                            }
                        }
                    })
                    .collect_view()
            }}
        </div>
    }
}
