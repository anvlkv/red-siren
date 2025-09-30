use leptos::prelude::*;
use leptos_use::UseRafFnCallbackArgs;
use shared::{
    instrument::GroupChanel,
    instrument::{StringSnoopDataRequest, StringSnoopDataResponse},
};
use tauri_use::{use_invoke, UseTauriReturn};

use crate::util::raf_fn_fps::use_raf_fn_with_fps;

#[component]
pub fn InstrumentStrings(
    #[prop(into)] layout: Signal<shared::instrument::Layout>,
) -> impl IntoView {
    let view_box = move || {
        let space = layout().space;
        format!("0 0 {} {}", space.x, space.y)
    };
    view! {
        <svg viewBox=view_box fill="none" xmlns="http://www.w3.org/2000/svg">
            {move || {
                let layout = layout();
                (0..layout.num_groups.get())
                    .flat_map(|g: u8| {
                        (0..layout.num_keys_per_group.get())
                            .map(move |k: u8| {
                                let k = k as usize;
                                let g = g as usize;
                                let ch = layout
                                    .first_group_channel
                                    .nth_channel_from_first(k + (g * k));
                                let orientation = layout.orientation;
                                let line = match ch {
                                    shared::instrument::GroupChanel::Left => {
                                        layout.left_string_position
                                    }
                                    shared::instrument::GroupChanel::Right => {
                                        layout.right_string_position
                                    }
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
    line: shared::Line,
    orientation: shared::orientation::LayoutOrientation,
    ch: GroupChanel,
) -> impl IntoView {
    let (start, end) = line;

    let path_def = Signal::derive(move || format!("M{} {} L{} {}", start.x, start.y, end.x, end.y));

    let UseTauriReturn {
        data,
        error,
        trigger,
    } = use_invoke::<StringSnoopDataRequest, (), StringSnoopDataResponse>(
        shared::instrument::data::GET_STRING_SNOOP_DATA,
    );

    // let _ = use_raf_fn_with_fps(move |UseRafFnCallbackArgs{ delta, timestamp }| {
    //     let (start, end) = line;

    //     if let Some(data) = data.get_untracked() {
    //         let path = match orientation {
    //             shared::orientation::LayoutOrientation::Vertical => crate::util::wave::waveform_path_y(
    //             samples,
    //             WAVES_LENGHT[i],
    //             WAVE_CENTER_X,
    //             base_y,
    //             WAVE_AMPLITUDE_PX,
    //         ),
    //             shared::orientation::LayoutOrientation::Horizontal => crate::util::wave::waveform_path_x(
    //             samples,
    //             WAVES_LENGHT[i],
    //             WAVE_CENTER_X,
    //             base_y,
    //             WAVE_AMPLITUDE_PX,
    //         ),
    //         };

    //     }

    // }, 30);

    view! { <path d=path_def /> }
}
