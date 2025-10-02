use leptos::prelude::*;
use common::{
    instrument::GroupChanel,
    instrument::{StringSnoopDataRequest, StringSnoopDataResponse},
};
use tauri_use::{use_invoke, UseTauriReturn};

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
        ..
    } = expect_layout_contex();

    let view_box = move || {
        let space = space();
        format!("0 0 {} {}", space.x, space.y)
    };
    view! {
        <svg viewBox=view_box fill="none" xmlns="http://www.w3.org/2000/svg">
            {move || {
                let num_groups = num_groups();
                let num_keys_per_group = num_keys_per_group();
                let first_group_channel = first_group_channel();
                let orientation = orientation();
                let left_string_position = left_string_position();
                let right_string_position = right_string_position();
                (0..num_groups)
                    .flat_map(|g: u8| {
                        (0..num_keys_per_group)
                            .map(move |k: u8| {
                                let k = k as usize;
                                let g = g as usize;
                                let ch = first_group_channel.nth_channel_from_first(g);
                                let orientation = orientation;
                                let line = match ch {
                                    common::instrument::GroupChanel::Left => left_string_position,
                                    common::instrument::GroupChanel::Right => right_string_position,
                                };

                                view! { <StringView g k line orientation ch /> }
                            })
                    })
                    .collect_view()
            }}
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
