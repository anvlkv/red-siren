use leptos::prelude::*;
use leptos_use::{use_raf_fn_with_options, UseRafFnOptions};
use tauri_use::{use_command, UseTauriWithReturn};

// Wave geometry (tiled, taller, centered under sun)
// Sun center (107,164), radius 39 => bottom ≈ 203 -> start just below.
// Wave tuning constants:
// - WAVE_TOP_Y: anchor just below sun
// - WAVE_VERTICAL_SPACING: distance between lines (increase to spread stack & lower bottom)
// - WAVE_Y_OFFSET: pushes whole stack downward
// - lengths[]: per-line half-width*2 (adjust individual widths; first narrower, last wider)
// Adjust here to refine geometry.
const WAVE_TOP_Y: f32 = 210.0; // anchor under sun
const WAVE_VERTICAL_SPACING: f32 = 64.0; // was 24
const WAVE_Y_OFFSET: f32 = 32.0; // pushes stack downward
const WAVE_AMPLITUDE_PX: f32 = 20.0;
const WAVE_CENTER_X: f32 = 107.0;
const WAVES_LENGHT: [f32; 11] = [
    48.0, 128.0, 232.0, 336.0, 440.0, 544.0, 648.0, 752.0, 856.0, 960.0, 1280.0,
];
const FIRST_WAVE_SCALE_X: f32 = 0.75;
const LAST_WAVE_SCALE_X: f32 = 0.95;

#[component]
pub fn Wavering() -> impl IntoView {
    // Pull-model intro DSP wave integration (MAYA DRY KISS) ---
    // Each RAF tick invokes backend command returning latest batch.
    use shared::commands::intro::INTRO_NEXT_FRAME;
    use shared::events::intro::IntroSnoopBatchPayload;

    // Invoke handle for next-frame command (no args).
    let UseTauriWithReturn {
        trigger: fetch_frame,
        data: batch,
        ..
    } = use_command::<IntroSnoopBatchPayload>(INTRO_NEXT_FRAME);

    // Cache of 11 SVG path strings for the animated wave lines.
    let wave_paths = RwSignal::<Vec<(String, String)>>::new(
        (0..11)
            .map(|i| {
                // Prepopulate with straight lines (flat at y=0)
                let samples = vec![0.0; 32];
                (
                    waveform_path(
                        &samples,
                        WAVES_LENGHT[i],
                        WAVE_CENTER_X,
                        WAVE_TOP_Y + WAVE_Y_OFFSET + (i as f32) * WAVE_VERTICAL_SPACING,
                        WAVE_AMPLITUDE_PX,
                    ),
                    wave_scale(i),
                )
            })
            .collect(),
    );

    let _wave_raf = use_raf_fn_with_options(
        {
            move |UseRafFnCallbackArgs {
                      delta: _,
                      timestamp: _,
                  }| {
                if let Some(batch) = batch.get() {
                    wave_paths.update(|paths| {
                        for (i, (snoop, (p, _))) in
                            batch.snoops.iter().zip(paths.iter_mut()).enumerate()
                        {
                            if i >= 11 {
                                break;
                            }
                            let base_y =
                                WAVE_TOP_Y + WAVE_Y_OFFSET + (i as f32) * WAVE_VERTICAL_SPACING;
                            let samples = &snoop.samples;
                            let path = waveform_path(
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
            }
        },
        UseRafFnOptions::default().immediate(true),
    );

    view! {
        <svg
            viewBox="0 0 1048 932"
            fill="none"
            class="splash-fragment waves stroke-gray dark:stroke-cinnabar blur-[.5px]"
            xmlns="http://www.w3.org/2000/svg"
        >
            {move || {
                wave_paths()
                    .into_iter()
                    .map(|(p, t)| {
                        view! {
                            <g transform=t>
                                <path d=p stroke-width="1.5" />
                            </g>
                        }
                            .into_any()
                    })
                    .collect_view()
            }}
            <g transform=format!(
                "translate(0,{})",
                WAVE_Y_OFFSET / 3.0,
            )>
                {move || {
                    wave_paths()
                        .into_iter()
                        .map(|(p, _)| view! { <path d=p stroke-width="1.5" /> }.into_any())
                        .collect_view()
                }}
            </g>
        </svg>
    }
}

// Render a single waveform path from the chronological samples.
fn waveform_path(
    samples: &[f32],
    total_len: f32,
    center_x: f32,
    center_y: f32,
    amp: f32,
) -> String {
    if samples.len() < 2 {
        return String::new();
    }
    let points = samples.len();
    let dx = total_len / (points - 1) as f32;
    let start_x = center_x - total_len * 0.5;
    let mut s = String::with_capacity(points * 12);
    for (i, &src) in samples.iter().enumerate() {
        let x = start_x + dx * i as f32;
        let y = center_y - src * amp;
        if i == 0 {
            s.push_str(&format!("M{:.2} {:.2}", x, y));
        } else {
            s.push_str(&format!("L{:.2} {:.2}", x, y));
        }
    }
    s
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
