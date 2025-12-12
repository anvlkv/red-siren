use std::collections::VecDeque;

use leptos::prelude::*;
use tauri_use::{use_command, UseTauriWithReturn};

use crate::util::{
    layout_context::{expect_layout_contex, LayoutContextReturn},
    raf_fn_fps::use_raf_fn_with_fps,
};

#[component]
pub fn SpectrumViz() -> impl IntoView {
    let LayoutContextReturn {
        space, orientation, ..
    } = expect_layout_contex();
    let UseTauriWithReturn {
        trigger: fetch_spectrum,
        data: spectrum_data,
        error: spectrum_error,
        ..
    } = use_command::<Option<Vec<(f32, f32)>>>(
        common::instrument::commands::SNAPSHOT_PROCESSED_OUTPUT_SPECTRUM,
    );

    let (visualize_spectrum, set_visualize_spectrum) = signal(VecDeque::<Vec<(f32, f32)>>::new());

    let cell_size = Memo::new(move |_| {
        let orientation = orientation();
        let space = space();
        visualize_spectrum()
            .iter()
            .map(|e| e.len() * 2)
            .max()
            .map(|mx| match orientation {
                common::orientation::LayoutOrientation::Vertical => space.x / mx as f64,
                common::orientation::LayoutOrientation::Horizontal => space.y / mx as f64,
            })
            .unwrap_or(14.0)
    });

    let history_length = Memo::new(move |_| {
        let space = space();
        let cell_size = cell_size();
        match orientation() {
            common::orientation::LayoutOrientation::Vertical => {
                (space.y / cell_size).ceil() as usize
            }
            common::orientation::LayoutOrientation::Horizontal => {
                (space.x / cell_size).ceil() as usize
            }
        }
    });

    let _raf = use_raf_fn_with_fps(
        move |_| {
            fetch_spectrum(Some(()));
            if let Some(data) = spectrum_data().flatten() {
                let history_length = history_length.get_untracked();
                set_visualize_spectrum.update(move |d| {
                    if d.capacity() < history_length {
                        d.reserve(history_length - d.capacity() + 1);
                    }

                    if d.len() >= history_length {
                        d.pop_front();
                    } else if d.is_empty() {
                        log::info!("first snapshot length: [{} x 2]", data.len());
                    }
                    d.push_back(data);
                });
            }
        },
        20.0,
    );

    Effect::new(move |_| {
        if let Some(err) = spectrum_error() {
            log::error!(
                "Error invoking {}: {err}",
                common::instrument::commands::SNAPSHOT_PROCESSED_OUTPUT_SPECTRUM
            );
        }
    });

    let view_box = move || {
        let space = space();
        format!("0 0 {} {}", space.x, space.y)
    };

    let cell_dimensions = Memo::new(move |_| {
        let history_length = history_length();
        let space = space();
        let base_size = cell_size();
        match orientation() {
            common::orientation::LayoutOrientation::Vertical => {
                (base_size, space.y / history_length as f64)
            }
            common::orientation::LayoutOrientation::Horizontal => {
                (space.x / history_length as f64, base_size)
            }
        }
    });

    Effect::new(move |_| {
        log::info!(
            "cell_dimensions=[{:?}]; history_length=[{}];",
            cell_dimensions(),
            history_length()
        )
    });

    view! {
        <svg viewBox=view_box fill="none" xmlns="http://www.w3.org/2000/svg">
            {move || {
                visualize_spectrum()
                    .into_iter()
                    .enumerate()
                    .map(|(h, data)| {
                        let orientation = orientation();
                        let space = space();
                        let width = space.x;
                        let height = space.y;
                        let (cell_width, cell_height) = cell_dimensions();

                        view! {
                            <SpectrumDataRow
                                orientation
                                width
                                height
                                cell_width
                                cell_height
                                row_index=h
                                data
                            />
                        }
                    })
                    .collect_view()
            }}
        </svg>
    }
}

#[component]
fn SpectrumDataRow(
    orientation: common::orientation::LayoutOrientation,
    width: f64,
    height: f64,
    cell_width: f64,
    cell_height: f64,
    row_index: usize,
    data: Vec<(f32, f32)>,
) -> impl IntoView {
    let transform = match orientation {
        common::orientation::LayoutOrientation::Vertical => {
            format!(
                "translate(0, {})",
                height - (row_index as f64 + 1.0) * cell_height
            )
        }
        common::orientation::LayoutOrientation::Horizontal => {
            format!(
                "translate({}, 0)",
                width - (row_index as f64 + 1.0) * cell_width
            )
        }
    };

    let cell_increment = match orientation {
        common::orientation::LayoutOrientation::Vertical => cell_width,
        common::orientation::LayoutOrientation::Horizontal => cell_height,
    };

    let right_base = match orientation {
        common::orientation::LayoutOrientation::Vertical => width / 2.0,
        common::orientation::LayoutOrientation::Horizontal => height / 2.0,
    };

    view! {
        <g transform=transform>
            {move || {
                data.iter()
                    .enumerate()
                    .map(|(i, &(l, r))| {
                        let i = i as f64;
                        match orientation {
                            common::orientation::LayoutOrientation::Vertical => {
                                view! {
                                    <g class="fill-cinnabar dark:fill-gray">
                                        <rect
                                            width=cell_width
                                            height=cell_height
                                            x=cell_increment * i
                                            y=0.0
                                            opacity=l
                                        />
                                        <rect
                                            width=cell_width
                                            height=cell_height
                                            x=cell_increment * i + right_base
                                            y=0.0
                                            opacity=r
                                        />
                                    </g>
                                }
                            }
                            common::orientation::LayoutOrientation::Horizontal => {
                                view! {
                                    <g class="fill-cinnabar dark:fill-gray">
                                        <rect
                                            width=cell_width
                                            height=cell_height
                                            x=0.0
                                            y=cell_increment * i
                                            opacity=l
                                        />
                                        <rect
                                            width=cell_width
                                            height=cell_height
                                            x=0.0
                                            y=cell_increment * i + right_base
                                            opacity=r
                                        />
                                    </g>
                                }
                            }
                        }
                    })
                    .collect_view()
            }}
        </g>
    }
}
