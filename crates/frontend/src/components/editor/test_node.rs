use common::instrument::data::{
    ExcitementSnoopBatchPayload, StringSnoopBatchPayload, GET_ALL_ACTIVATION_SNOOPS,
    GET_ALL_STRING_SNOOPS,
};
use common::{
    commands::test_node::{
        TestNodeHitPayload, TestNodeReleasePayload, TEST_NODE_HIT, TEST_NODE_RELEASE,
    },
    NodeKey,
};
use leptos::prelude::*;
use tauri_use::{use_command, use_invoke, UseTauriReturn, UseTauriWithReturn};

use crate::{
    components::{Fold, RangeSlider, SliderValue},
    util::raf_fn_fps::use_raf_fn_with_fps,
};

const FREQ_MIN: f32 = 20.0;
const FREQ_MAX: f32 = 4_000.0;
const FREQ_STEP: f32 = 1.0;
const FREQ_DEFAULT: f32 = 440.0;

const EXCITE_MIN: f32 = 0.0;
const EXCITE_MAX: f32 = 1.0;
const EXCITE_STEP: f32 = 0.01;

const SNOOP_FPS: f64 = 12.0;

/// √-compressed mean-absolute-value, clamped to [0, 1].
/// Same algorithm as `mean_abs_clamped` in `instrument/debug.rs`.
fn mean_abs_clamped(samples: Option<Vec<f32>>) -> f32 {
    if let Some(s) = samples {
        if !s.is_empty() {
            let mean = s.iter().map(|x| x.abs()).sum::<f32>() / s.len() as f32;
            return mean.sqrt().clamp(0.0, 1.0);
        }
    }
    0.0
}

/// Build SVG `points` for a polyline inside a `viewBox="0 0 300 40"`.
/// x ∈ [0, 300], y centred on 20 with ±18 amplitude.
fn waveform_points(samples: &[f32]) -> String {
    let n = samples.len();
    if n < 2 {
        return String::new();
    }
    let denom = (n - 1) as f32;
    samples
        .iter()
        .enumerate()
        .map(|(i, &s)| {
            let x = i as f32 / denom * 300.0;
            let y = 20.0 - s.clamp(-1.0, 1.0) * 18.0;
            format!("{x:.1},{y:.1}")
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Devtools panel for manually exercising a single audio node.
///
/// Sections:
/// - **Node** — band + key via sliders.
/// - **Parameters** — frequency and excite (re/im) sliders.
/// - **Hold-to-play** pad — mousedown → hit, mouseup / mouseleave → release.
/// - **Snoop** — A1 / A2 / O level bars and an output waveform polled at 12 FPS.
#[component]
pub fn TestNodePanel() -> impl IntoView {
    // ── node selection ────────────────────────────────────────────────────────
    let (node_band, set_node_band) = signal(0_u8);
    let (node_key_idx, set_node_key_idx) = signal(0_u8);

    // ── audio parameters ──────────────────────────────────────────────────────
    let (frequency, set_frequency) = signal(FREQ_DEFAULT);
    let (excite_real, set_excite_real) = signal(1.0_f32);
    let (excite_imag, set_excite_imag) = signal(0.0_f32);

    // ── hit / release ────────────────────────────────────────────────────────
    let UseTauriReturn {
        trigger: do_hit,
        error: hit_error,
        ..
    } = use_invoke::<TestNodeHitPayload, (), ()>(TEST_NODE_HIT);

    let UseTauriReturn {
        trigger: do_release,
        error: release_error,
        ..
    } = use_invoke::<TestNodeReleasePayload, (), ()>(TEST_NODE_RELEASE);

    // ── snoop batch commands ─────────────────────────────────────────────────
    let UseTauriWithReturn {
        trigger: fetch_excitement,
        data: excitement_batch,
        error: excitement_error,
        ..
    } = use_command::<ExcitementSnoopBatchPayload>(GET_ALL_ACTIVATION_SNOOPS);

    let UseTauriWithReturn {
        trigger: fetch_output,
        data: output_batch,
        error: output_error,
        ..
    } = use_command::<StringSnoopBatchPayload>(GET_ALL_STRING_SNOOPS);

    // ── error logging ────────────────────────────────────────────────────────
    Effect::new(move |_| {
        if let Some(err) = hit_error() {
            log::error!("Error invoking {TEST_NODE_HIT}: {err}");
        }
        if let Some(err) = release_error() {
            log::error!("Error invoking {TEST_NODE_RELEASE}: {err}");
        }
        if let Some(err) = excitement_error() {
            log::error!("Error invoking {GET_ALL_ACTIVATION_SNOOPS}: {err}");
        }
        if let Some(err) = output_error() {
            log::error!("Error invoking {GET_ALL_STRING_SNOOPS}: {err}");
        }
    });

    // ── snoop RAF poll (same pattern as DebugOverlay) ────────────────────────
    let _raf = use_raf_fn_with_fps(
        move |_| {
            fetch_excitement(Some(()));
            fetch_output(Some(()));
        },
        SNOOP_FPS,
    );

    // ── handlers ─────────────────────────────────────────────────────────────
    let on_hit = move |_: web_sys::MouseEvent| {
        do_hit(Some((
            TestNodeHitPayload {
                node_key: NodeKey(node_band(), node_key_idx()),
                frequency: frequency(),
                excite_real: excite_real(),
                excite_imag: excite_imag(),
            },
            (),
        )));
    };

    let on_release = move |_: web_sys::MouseEvent| {
        do_release(Some((
            TestNodeReleasePayload {
                node_key: NodeKey(node_band(), node_key_idx()),
            },
            (),
        )));
    };

    // ── derived snoop signals for the selected node ───────────────────────────
    // Re-channel each time group/key changes; excitement_batch polled at SNOOP_FPS.
    let excite_real_samples = Signal::derive(move || {
        excitement_batch().and_then(|b| {
            b.snoops
                .into_iter()
                .find(|e| e.band == node_band() && e.key == node_key_idx())
                .map(|e| e.samples.into_iter().map(|(r, _)| r).collect::<Vec<_>>())
        })
    });

    let excite_imag_samples = Signal::derive(move || {
        excitement_batch().and_then(|b| {
            b.snoops
                .into_iter()
                .find(|e| e.band == node_band() && e.key == node_key_idx())
                .map(|e| e.samples.into_iter().map(|(_, i)| i).collect::<Vec<_>>())
        })
    });

    let output_samples = Signal::derive(move || {
        output_batch().and_then(|b| {
            b.snoops
                .into_iter()
                .find(|e| e.band == node_band() && e.key == node_key_idx())
                .map(|e| e.samples)
        })
    });

    // Level memos — same √-compressed mean-abs as PairTile in debug.rs
    let a1_level = Memo::new(move |_| mean_abs_clamped(excite_real_samples()));
    let a2_level = Memo::new(move |_| mean_abs_clamped(excite_imag_samples()));
    let out_level = Memo::new(move |_| mean_abs_clamped(output_samples()));

    // Bar-width style strings (same pattern as PairTile)
    let a1_w = move || format!("width: {:.0}%;", (a1_level() * 100.0).clamp(0.0, 100.0));
    let a2_w = move || format!("width: {:.0}%;", (a2_level() * 100.0).clamp(0.0, 100.0));
    let out_w = move || format!("width: {:.0}%;", (out_level() * 100.0).clamp(0.0, 100.0));

    // SVG waveform points (viewBox 0 0 300 40)
    let waveform_pts =
        Signal::derive(move || waveform_points(&output_samples().unwrap_or_default()));

    // ── view ──────────────────────────────────────────────────────────────────
    view! {
        <div class="w-full h-full min-h-0 overflow-auto px-4 py-4 md:px-8 md:py-6">
            <div class="mx-auto flex w-full max-w-xl flex-col gap-6">

                // ── Node ──────────────────────────────────────────────────────
                <Fold title="Node">
                    <div class="flex flex-col gap-3 pt-2">
                        <RangeSlider
                            label="Band"
                            show_value=true
                            value=Signal::derive(move || SliderValue::Single(node_band() as f32))
                            min=0.0_f32
                            max=15.0_f32
                            step=1.0_f32
                            on_input=Callback::new(move |v: SliderValue| {
                                let n: f32 = v.into();
                                set_node_band(n as u8);
                            })
                        />
                        <RangeSlider
                            label="Key"
                            show_value=true
                            value=Signal::derive(move || SliderValue::Single(node_key_idx() as f32))
                            min=0.0_f32
                            max=15.0_f32
                            step=1.0_f32
                            on_input=Callback::new(move |v: SliderValue| {
                                let n: f32 = v.into();
                                set_node_key_idx(n as u8);
                            })
                        />
                        <p class="text-sm tabular-nums text-black/50 dark:text-red/50">
                            {move || format!("NodeKey({}, {})", node_band(), node_key_idx())}
                        </p>
                    </div>
                </Fold>

                // ── Parameters ───────────────────────────────────────────────
                <Fold title="Parameters">
                    <div class="flex flex-col gap-3 pt-2">
                        <RangeSlider
                            label="Frequency (Hz)"
                            show_value=true
                            value=Signal::derive(move || SliderValue::Single(frequency()))
                            min=FREQ_MIN
                            max=FREQ_MAX
                            step=FREQ_STEP
                            on_input=Callback::new(move |v: SliderValue| {
                                set_frequency(v.into());
                            })
                        />
                        <RangeSlider
                            label="Excite — real"
                            show_value=true
                            value=Signal::derive(move || SliderValue::Single(excite_real()))
                            min=EXCITE_MIN
                            max=EXCITE_MAX
                            step=EXCITE_STEP
                            on_input=Callback::new(move |v: SliderValue| {
                                set_excite_real(v.into());
                            })
                        />
                        <RangeSlider
                            label="Excite — imaginary"
                            show_value=true
                            value=Signal::derive(move || SliderValue::Single(excite_imag()))
                            min=EXCITE_MIN
                            max=EXCITE_MAX
                            step=EXCITE_STEP
                            on_input=Callback::new(move |v: SliderValue| {
                                set_excite_imag(v.into());
                            })
                        />
                    </div>
                </Fold>

                // ── Hold-to-play pad ─────────────────────────────────────────
                <div class="flex w-full justify-center">
                    <div
                        class="flex h-24 w-48 cursor-pointer select-none items-center \
                         justify-center rounded-2xl border-2 border-black/20 \
                         bg-black/5 text-sm font-semibold tracking-wide \
                         transition-colors active:bg-black/15 \
                         dark:border-red/30 dark:bg-red/5 dark:active:bg-red/15"
                        on:mousedown=on_hit
                        on:mouseup=on_release
                        on:mouseleave=on_release
                    >
                        "Hold to play"
                    </div>
                </div>

                // ── Snoop ────────────────────────────────────────────────────
                <Fold title="Snoop" open=false>
                    <div class="flex flex-col gap-3 pt-2">

                        // current node label
                        <p class="text-xs tabular-nums text-black/50 dark:text-red/50">
                            {move || format!("NodeKey({}, {})", node_band(), node_key_idx())}
                        </p>

                        // A1 — excite real
                        <div>
                            <div class="h-[5px] w-full rounded bg-black/15 dark:bg-red/15">
                                <div
                                    class="h-[5px] rounded bg-black dark:bg-red \
                                     transition-[width] duration-75"
                                    style=a1_w
                                />
                            </div>
                            <div class="mt-[3px] flex justify-between">
                                <span class="text-xs text-black/50 dark:text-red/50">
                                    "A1 (re)"
                                </span>
                                <span class="text-xs tabular-nums text-black/50 dark:text-red/50">
                                    {move || format!("{:.2}", a1_level())}
                                </span>
                            </div>
                        </div>

                        // A2 — excite imag
                        <div>
                            <div class="h-[5px] w-full rounded bg-black/15 dark:bg-red/15">
                                <div
                                    class="h-[5px] rounded bg-black/60 dark:bg-red/60 \
                                     transition-[width] duration-75"
                                    style=a2_w
                                />
                            </div>
                            <div class="mt-[3px] flex justify-between">
                                <span class="text-xs text-black/50 dark:text-red/50">
                                    "A2 (im)"
                                </span>
                                <span class="text-xs tabular-nums text-black/50 dark:text-red/50">
                                    {move || format!("{:.2}", a2_level())}
                                </span>
                            </div>
                        </div>

                        // O — output level
                        <div>
                            <div class="h-[5px] w-full rounded bg-black/15 dark:bg-red/15">
                                <div
                                    class="h-[5px] rounded bg-black/40 dark:bg-red/40 \
                                     transition-[width] duration-75"
                                    style=out_w
                                />
                            </div>
                            <div class="mt-[3px] flex justify-between">
                                <span class="text-xs text-black/50 dark:text-red/50">
                                    "O (out)"
                                </span>
                                <span class="text-xs tabular-nums text-black/50 dark:text-red/50">
                                    {move || format!("{:.2}", out_level())}
                                </span>
                            </div>
                        </div>

                        // Output waveform
                        <div class="w-full overflow-hidden rounded \
                         border border-black/10 dark:border-red/10 \
                         bg-black/5 dark:bg-red/5">
                            <svg
                                viewBox="0 0 300 40"
                                preserveAspectRatio="none"
                                class="w-full h-16"
                                xmlns="http://www.w3.org/2000/svg"
                            >
                                // centre line
                                <line
                                    x1="0"
                                    y1="20"
                                    x2="300"
                                    y2="20"
                                    stroke="currentColor"
                                    stroke-width="0.4"
                                    class="text-black/20 dark:text-red/20"
                                />
                                // waveform
                                <polyline
                                    points=move || waveform_pts()
                                    fill="none"
                                    stroke="currentColor"
                                    stroke-width="1.2"
                                    stroke-linejoin="round"
                                    stroke-linecap="round"
                                    class="text-black dark:text-red"
                                />
                            </svg>
                        </div>

                    </div>
                </Fold>

            </div>
        </div>
    }
}
