use common::{
    instrument::GroupChanel,
    instrument::{StringSnoopDataRequest, StringSnoopDataResponse},
};
use leptos::prelude::*;
use tauri_use::{use_invoke, UseTauriReturn};

use crate::components::intro::consts::INTRO_SUN_RADIUS;
use crate::util::layout_context::{expect_layout_contex, LayoutContextReturn};

#[component]
pub fn InstrumentStrings() -> impl IntoView {
    let LayoutContextReturn {
        space,
        num_groups,
        num_keys_per_group,
        first_group_channel,
        orientation,
        left_string_position,
        right_string_position,
        key_radius,
        ..
    } = expect_layout_contex();
    let ctx_left = super::expect_instrument_context();
    let ctx_right = ctx_left.clone();

    let view_box = move || {
        let space = space();
        format!("0 0 {} {}", space.x, space.y)
    };

    let left_strings = move || {
        let num_groups = num_groups();
        let num_keys_per_group = num_keys_per_group();
        let first_group_channel = first_group_channel();
        let orientation = orientation();
        let left_string_position = left_string_position();

        // Animate only the channel <g>, not inner elements.
        // Compute a simple channel-level bbox from the shared line endpoints and guard first-time.
        let (start, end) = left_string_position;
        let left = start.x.min(end.x);
        let top = start.y.min(end.y);
        let right = start.x.max(end.x);
        let bottom = start.y.max(end.y);
        let width = right - left;
        let height = bottom - top;
        let first_time = {
            let bbox = super::context::Bounding {
                x: left,
                y: top,
                width,
                height,
                top,
                right,
                bottom,
                left,
            };
            // Use a fixed channel key (0 = left) for one-shot animation
            ctx_left.upsert_strings_group_rect(0, bbox)
        };
        let class = if first_time {
            super::instrument_animations::INSTRUMENT_STRINGS_GROUP_APPEAR.to_string()
        } else {
            String::new()
        };
        let scale = INTRO_SUN_RADIUS / key_radius();
        let style = format!("--inst-strings-appear-delay: 0ms; --inst-strings-k1-tx: -{}px; --inst-strings-k1-ty: -{}px; --inst-strings-k1-scale: {};", left, top, scale);
        view! {
            <g id="left-channel-strings" attr:class=class attr:style=style>
                {move || {
                    (0..num_groups)
                        .filter(|g| {
                            matches!(
                                first_group_channel.nth_channel_from_first(*g as usize),
                                common::instrument::GroupChanel::Left
                            )
                        })
                        .flat_map(|g: u8| {
                            (0..num_keys_per_group)
                                .map(move |k: u8| {
                                    let k = k as usize;
                                    let g = g as usize;
                                    let ch = first_group_channel.nth_channel_from_first(g);
                                    let orientation = orientation;

                                    view! {
                                        <StringView g k line=left_string_position orientation ch />
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
        let orientation = orientation();
        let right_string_position = right_string_position();

        // Animate only the channel <g>, not inner elements.
        // Compute a simple channel-level bbox from the shared line endpoints and guard first-time.
        let (start, end) = right_string_position;
        let left = start.x.min(end.x);
        let top = start.y.min(end.y);
        let right = start.x.max(end.x);
        let bottom = start.y.max(end.y);
        let width = right - left;
        let height = bottom - top;
        let first_time = {
            let bbox = super::context::Bounding {
                x: left,
                y: top,
                width,
                height,
                top,
                right,
                bottom,
                left,
            };
            // Use a fixed channel key (1 = right) for one-shot animation
            ctx_right.upsert_strings_group_rect(1, bbox)
        };
        let class = if first_time {
            super::instrument_animations::INSTRUMENT_STRINGS_GROUP_APPEAR.to_string()
        } else {
            String::new()
        };
        let scale = INTRO_SUN_RADIUS / key_radius();
        let style = format!("--inst-strings-appear-delay: 0ms; --inst-strings-k1-tx: -{}px; --inst-strings-k1-ty: -{}px; --inst-strings-k1-scale: {};", left, top, scale);
        view! {
            <g id="right-channel-strings" attr:class=class attr:style=style>
                {move || {
                    (0..num_groups)
                        .filter(|g| {
                            matches!(
                                first_group_channel.nth_channel_from_first(*g as usize),
                                common::instrument::GroupChanel::Right
                            )
                        })
                        .flat_map(|g: u8| {
                            (0..num_keys_per_group)
                                .map(move |k: u8| {
                                    let k = k as usize;
                                    let g = g as usize;
                                    let ch = first_group_channel.nth_channel_from_first(g);
                                    let orientation = orientation;

                                    view! {
                                        <StringView g k line=right_string_position orientation ch />
                                    }
                                })
                        })
                        .collect_view()
                }}

            </g>
        }
    };

    view! {
        <svg viewBox=view_box fill="none" xmlns="http://www.w3.org/2000/svg">
            {left_strings}
            {right_strings}
        </svg>
    }
}

#[component]
pub fn StringView(
    g: usize,
    k: usize,
    line: common::Line,
    orientation: common::orientation::LayoutOrientation,
    ch: GroupChanel,
) -> impl IntoView {
    let (start, end) = line;

    let path_def = Signal::derive(move || format!("M{},{} L{},{}", start.x, start.y, end.x, end.y));

    let UseTauriReturn {
        data,
        error,
        trigger,
    } = use_invoke::<StringSnoopDataRequest, (), StringSnoopDataResponse>(
        common::instrument::data::GET_STRING_SNOOP_DATA,
    );

    // let _ = use_raf_fn_with_fps(move |UseRafFnCallbackArgs{ delta, timestamp }| {
    //     let (start, end) = line;

    //     if let Some(data) = data.get_untracked() {
    //         let path = match orientation {
    //             common::orientation::LayoutOrientation::Vertical => crate::util::wave::waveform_path_y(
    //             samples,
    //             WAVES_LENGHT[i],
    //             WAVE_CENTER_X,
    //             base_y,
    //             WAVE_AMPLITUDE_PX,
    //         ),
    //             common::orientation::LayoutOrientation::Horizontal => crate::util::wave::waveform_path_x(
    //             samples,
    //             WAVES_LENGHT[i],
    //             WAVE_CENTER_X,
    //             base_y,
    //             WAVE_AMPLITUDE_PX,
    //         ),
    //         };

    //     }

    // }, 30);

    view! { <path d=path_def stroke-width="2" /> }
}
