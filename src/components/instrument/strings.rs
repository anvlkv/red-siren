use common::instrument::GroupChannel;
use leptos::prelude::*;
use tauri_use::{use_command, UseTauriWithReturn};

use crate::components::intro::consts::{INTRO_FLUTE_POS_X, INTRO_FLUTE_POS_Y, INTRO_FLUTE_ROT_DEG};
use crate::util::layout_context::{expect_layout_contex, LayoutContextReturn};

#[component]
pub fn InstrumentStrings() -> impl IntoView {
    let LayoutContextReturn {
        space,
        num_groups,
        num_keys_per_group,
        first_group_channel,
        left_string_position,
        right_string_position,
        complete_layout,
        ..
    } = expect_layout_contex();

    let string_wave_amplitude = Memo::new(move |_| complete_layout().instrument_breadth * 0.55);

    // Instrument snoop batch stream (pull model).
    let UseTauriWithReturn {
        trigger: fetch_batch,
        data: batch_data,
        error: batch_error,
        ..
    } = use_command::<common::instrument::data::StringSnoopBatchPayload>(
        common::instrument::data::GET_ALL_STRING_SNOOPS,
    );

    // Log errors for the batch command
    Effect::new(move |_| {
        if let Some(err) = batch_error() {
            log::error!(
                "Error invoking {}: {err}",
                common::instrument::data::GET_ALL_STRING_SNOOPS
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

    // Root-level viewBox matches layout space; transforms use view-box coords via 'transform-box: view-box'
    let view_box = move || {
        let space = space();
        format!("0 0 {} {}", space.x, space.y)
    };

    // Inner: rotation animation origin + variables
    let root_inner_style = move || {
        // Map artwork (intro composition) coordinates to current layout space
        let viewport_w = crate::components::intro::consts::INTRO_COMP_VIEWBOX_WIDTH;
        let viewport_h = crate::components::intro::consts::INTRO_COMP_VIEWBOX_HEIGHT;
        let layout = space();
        let scale_x = layout.x / viewport_w;
        let scale_y = layout.y / viewport_h;
        let viewport_scale = scale_x.min(scale_y);
        let viewport_scale_x = 1.0 - scale_x;
        let viewport_scale_y = 1.0 - scale_y;

        // Transform origin aligned with flute rotation pivot in screen/layout space, using bottom-right anchoring of the composition:
        // - Scale = min(space/viewBox)
        // - Top-left of the scaled composition in screen coords = (space.x - scaled_w, space.y - scaled_h)
        // - Pivot in screen coords = offset + pivot_in_viewBox * scale
        let scaled_w = viewport_w * viewport_scale;
        let scaled_h = viewport_h * viewport_scale;
        let offset_x = layout.x - scaled_w;
        let offset_y = layout.y - scaled_h;
        let origin_x = offset_x + INTRO_FLUTE_POS_X * viewport_scale;
        let origin_y = offset_y + INTRO_FLUTE_POS_Y * viewport_scale;

        // Start rotated to match flute angle, then normalize to 0deg
        let k1_rot = INTRO_FLUTE_ROT_DEG;

        // Compute initial translation so the strings group moves from the flute pivot to its final place.
        // Anchor = midpoint between channel midpoints (in screen/layout space).
        let (ls, le) = left_string_position();
        let (rs, re) = right_string_position();
        let left_mid_x = (ls.x + le.x) / 2.0;
        let left_mid_y = (ls.y + le.y) / 2.0;
        let right_mid_x = (rs.x + re.x) / 2.0;
        let right_mid_y = (rs.y + re.y) / 2.0;
        let anchor_x = (left_mid_x + right_mid_x) / 2.0;
        let anchor_y = (left_mid_y + right_mid_y) / 2.0;

        let k1_tx = origin_x - anchor_x;
        let k1_ty = origin_y - anchor_y;

        format!(
            concat!(
                "transform-box: view-box;",
                " transform-origin: {}px {}px;",
                " --inst-strings-k1-rot: {}deg;",
                " --inst-strings-k1-tx: {}px;",
                " --inst-strings-k1-ty: {}px;",
                " --inst-strings-k1-scale-x: {};",
                " --inst-strings-k1-scale-y: {};",
            ),
            origin_x, origin_y, k1_rot, k1_tx, k1_ty, viewport_scale_x, viewport_scale_y,
        )
    };

    let left_strings = move || {
        let num_groups = num_groups();
        let num_keys_per_group = num_keys_per_group();
        let first_group_channel = first_group_channel();
        let left_string_position = left_string_position();

        view! {
            <g id="left-channel-strings">
                {move || {
                    (0..num_groups)
                        .filter(|g| {
                            matches!(
                                first_group_channel.nth_channel_from_first(*g as usize),
                                common::instrument::GroupChannel::Left
                            )
                        })
                        .flat_map(|g: u8| {
                            (0..num_keys_per_group)
                                .map(move |k: u8| {
                                    let k = k as usize;
                                    let g = g as usize;
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
                                        <StringView
                                            line=left_string_position
                                            samples=samples
                                            amplitude=string_wave_amplitude
                                        />
                                    }
                                })
                        })
                        .collect_view()
                }}
            </g>
        }
    };

    let right_strings = move || {
        let num_groups = num_groups();
        let num_keys_per_group = num_keys_per_group();
        let first_group_channel = first_group_channel();
        let right_string_position = right_string_position();

        view! {
            <g id="right-channel-strings">
                {move || {
                    (0..num_groups)
                        .filter(|g| {
                            matches!(
                                first_group_channel.nth_channel_from_first(*g as usize),
                                common::instrument::GroupChannel::Right
                            )
                        })
                        .flat_map(|g: u8| {
                            (0..num_keys_per_group)
                                .map(move |k: u8| {
                                    let k = k as usize;
                                    let g = g as usize;
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
                                        <StringView
                                            line=right_string_position
                                            samples=samples
                                            amplitude=string_wave_amplitude
                                        />
                                    }
                                })
                        })
                        .collect_view()
                }}
            </g>
        }
    };

    view! {
        <svg
            viewBox=view_box
            class="absolute h-full w-auto top-auto left-auto bottom-0 right-0 stroke-black dark:stroke-red"
            fill="none"
            xmlns="http://www.w3.org/2000/svg"
        >
            <g
                id="strings-root"
                class=super::instrument_animations::INSTRUMENT_STRINGS_ROOT_APPEAR
                style=root_inner_style
            >
                {left_strings}
                {right_strings}
            </g>
        </svg>
    }
}

#[component]
pub fn StringView(
    line: common::Line,
    #[prop(into)] samples: Signal<Option<Vec<f32>>>,
    #[prop(into)] amplitude: Signal<f32>,
) -> impl IntoView {
    let path_def = Signal::derive(move || {
        let (start, end) = line;
        if let Some(samples) = samples.get() {
            if !samples.is_empty() {
                return crate::util::wave::waveform_path_along_line(
                    &samples,
                    start,
                    end,
                    amplitude(),
                );
            }
        }
        format!("M{},{} L{},{}", start.x, start.y, end.x, end.y)
    });

    view! { <path d=path_def stroke-width="2" /> }
}
