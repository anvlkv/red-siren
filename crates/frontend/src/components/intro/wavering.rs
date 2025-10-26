use common::events::intro::IntroSnoopBatchPayload;
use leptos::prelude::*;
use tauri_use::{use_command, UseTauriWithReturn};

use crate::util::{
    raf_fn_fps::{use_raf_fn_with_fps, UseRafFnCallbackArgs},
    secondary_window::is_secondary_window,
};

// Wave geometry (tiled, taller, centered under sun)
const WAVE_TOP_Y: f32 = 210.0; // anchor under sun
const WAVE_VERTICAL_SPACING: f32 = 64.0;
const WAVE_Y_OFFSET: f32 = 32.0;
const WAVE_AMPLITUDE_PX: f32 = 20.0;
const WAVE_CENTER_X: f32 = 107.0;
const WAVES_LENGHT: [f32; 11] = [
    48.0, 128.0, 232.0, 336.0, 440.0, 544.0, 648.0, 752.0, 856.0, 960.0, 1280.0,
];
const FIRST_WAVE_SCALE_X: f32 = 0.75;
const LAST_WAVE_SCALE_X: f32 = 0.95;
const STROKE_WIDTH: f32 = 3.5;

#[component]
pub fn Wavering(#[prop(into)] paused: Signal<bool>) -> impl IntoView {
    let is_secondary_window = is_secondary_window();

    let UseTauriWithReturn {
        trigger: trigger_resume,
        error: resume_error,
        ..
    } = use_command::<()>(common::commands::intro::INTRO_RESUME);

    let UseTauriWithReturn {
        trigger: trigger_pause,
        error: pause_error,
        ..
    } = use_command::<()>(common::commands::intro::INTRO_PAUSE);

    let UseTauriWithReturn {
        trigger: fetch_frame,
        data: frame_data,
        error: frame_error,
        ..
    } = use_command::<IntroSnoopBatchPayload>(common::commands::intro::INTRO_NEXT_FRAME);

    // Log errors for frame / pause / resume commands
    Effect::new(move |_| {
        if let Some(err) = frame_error() {
            log::error!(
                "Error invoking {}: {err}",
                common::commands::intro::INTRO_NEXT_FRAME
            );
        }
        if let Some(err) = resume_error() {
            log::error!(
                "Error invoking {}: {err}",
                common::commands::intro::INTRO_RESUME
            );
        }
        if let Some(err) = pause_error() {
            log::error!(
                "Error invoking {}: {err}",
                common::commands::intro::INTRO_PAUSE
            );
        }
    });

    Effect::new(move |_| {
        if is_secondary_window() {
            return;
        }
        if paused() {
            trigger_pause(Some(()));
        } else {
            trigger_resume(Some(()));
        }
    });

    // Cache of 11 SVG path strings for the animated wave lines.
    let wave_paths = RwSignal::<Vec<(String, String, String)>>::new(
        (0..11)
            .map(|i| {
                let samples = vec![0.0; 32];
                (
                    crate::util::wave::waveform_path_x(
                        &samples,
                        WAVES_LENGHT[i],
                        WAVE_CENTER_X,
                        WAVE_TOP_Y + WAVE_Y_OFFSET + (i as f32) * WAVE_VERTICAL_SPACING,
                        WAVE_AMPLITUDE_PX,
                    ),
                    wave_scale(i),
                    wave_stroke(i),
                )
            })
            .collect(),
    );

    // RAF-driven waveform updates (paused/resumed by backend sampling commands)
    let _wave_raf = use_raf_fn_with_fps(
        move |UseRafFnCallbackArgs {
                  delta: _,
                  timestamp: _,
              }| {
            if let Some(batch) = frame_data.get() {
                wave_paths.update(|paths| {
                    for (i, (snoop, (p, _, _))) in
                        batch.snoops.iter().zip(paths.iter_mut()).enumerate()
                    {
                        if i >= 11 {
                            break;
                        }
                        let base_y =
                            WAVE_TOP_Y + WAVE_Y_OFFSET + (i as f32) * WAVE_VERTICAL_SPACING;
                        let samples = &snoop.samples;
                        let path = crate::util::wave::waveform_path_x(
                            samples,
                            WAVES_LENGHT[i],
                            WAVE_CENTER_X,
                            base_y,
                            WAVE_AMPLITUDE_PX,
                        );
                        *p = path;
                    }
                });
            }
            fetch_frame(Some(()));
        },
        20.0,
    );

    view! {
        <g>
            {move || {
                wave_paths()
                    .into_iter()
                    .map(|(p, t, s)| view! { <path d=p transform=t stroke-width=s /> }.into_any())
                    .collect_view()
            }}
            <g transform=format!(
                "translate(0,{})",
                WAVE_Y_OFFSET / 3.0,
            )>
                {move || {
                    wave_paths()
                        .into_iter()
                        .map(|(p, _, s)| view! { <path d=p stroke-width=s /> }.into_any())
                        .collect_view()
                }}
            </g>
        </g>
    }
}

fn wave_scale(i: usize) -> String {
    format!(
        "translate({:.2},{:.2}) scale({:.4},1) translate({:.2},{:.2})",
        WAVE_CENTER_X,
        { WAVE_TOP_Y + WAVE_Y_OFFSET + (i as f32) * WAVE_VERTICAL_SPACING },
        {
            let idx = i as f32;
            let max_idx = (WAVES_LENGHT.len().saturating_sub(1)) as f32;
            if max_idx == 0.0 {
                FIRST_WAVE_SCALE_X
            } else {
                FIRST_WAVE_SCALE_X + (LAST_WAVE_SCALE_X - FIRST_WAVE_SCALE_X) * (idx / max_idx)
            }
        },
        -WAVE_CENTER_X,
        -(WAVE_TOP_Y + WAVE_Y_OFFSET + (i as f32) * WAVE_VERTICAL_SPACING),
    )
}

fn wave_stroke(i: usize) -> String {
    let max_idx = WAVES_LENGHT.len() as f32;
    let width = (STROKE_WIDTH / max_idx) * (i as f32 + 1.0);
    format!("{width}")
}
