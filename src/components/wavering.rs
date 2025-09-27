use leptos::prelude::*;
use leptos_use::{use_raf_fn_with_options, UseRafFnOptions};
use tauri_use::{use_invoke, UseTauriReturn};

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
    64.0, 128.0, 232.0, 336.0, 440.0, 544.0, 648.0, 752.0, 856.0, 960.0, 1280.0,
];

#[component]
pub fn Wavering() -> impl IntoView {
    // --- Channel-based intro DSP wave integration (MAYA DRY KISS) ---
    // Local imports (scoped) to avoid polluting the broader module namespace.
    use crate::util::channel::{invoke_channel_only, use_typed_channel};
    use shared::commands::intro::INTRO_STREAM;
    use shared::events::intro::IntroSnoopBatchPayload;

    // Envelope { event, data } as emitted by backend intro engine.
    #[derive(Clone, serde::Deserialize)]
    struct IntroBatchEnvelope {
        event: String,
        data: IntroSnoopBatchPayload,
    }

    // Latest envelope from backend + the JS channel handle.
    let (snoop_env, channel_js) = use_typed_channel::<IntroBatchEnvelope>();

    // Start backend stream exactly once (idempotent; backend swaps channel if re-invoked).
    Effect::new({
        let channel_js = channel_js.clone();
        move |_| {
            invoke_channel_only(INTRO_STREAM, "onEvent", channel_js.clone());
        }
    });

    // Cache of 11 SVG path strings for the animated wave lines.
    let wave_paths = RwSignal::new(vec![String::new(); 11]);

    // RAF-driven renderer (separate from animation tween RAF) – keeps drawing smooth
    // even if backend batch rate is lower than monitor refresh.
    let _wave_raf = use_raf_fn_with_options(
        {
            move |_| {
                if let Some(env) = snoop_env.get() {
                    let batch = &env.data;

                    // Render a single waveform path from the chronological samples (no repetition).
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

                    let mut new_paths = Vec::with_capacity(11);
                    for (i, snoop) in batch.snoops.iter().enumerate() {
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
                        new_paths.push(path);
                    }
                    if new_paths.len() == 11 {
                        wave_paths.set(new_paths);
                    }
                }
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
                    .map(|p| view! { <path d=p stroke-width="1.5" /> }.into_any())
                    .collect_view()
            }}
            <g transform=format!(
                "translate(0,{})",
                WAVE_Y_OFFSET / 3.0,
            )>
                {move || {
                    wave_paths()
                        .into_iter()
                        .map(|p| view! { <path d=p stroke-width="1.5" /> }.into_any())
                        .collect_view()
                }}
            </g>
        </svg>
    }
}
