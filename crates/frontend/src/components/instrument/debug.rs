use leptos::prelude::*;
use tauri_use::{use_command, UseTauriWithReturn};

use crate::{
    components::Fold,
    util::layout_context::{expect_layout_contex, LayoutContextReturn},
};

const REFRESH_FPS: f64 = 12.0; // small & cheap; good enough to spot trends

#[component]
pub fn DebugOverlay() -> impl IntoView {
    let LayoutContextReturn {
        num_groups,
        num_keys_per_group,
        ..
    } = expect_layout_contex();

    // Pull excitement batches
    let UseTauriWithReturn {
        trigger: fetch_excitement,
        data: excitement_batch,
        error: excitement_error,
        ..
    } = use_command::<common::instrument::data::ExcitementSnoopBatchPayload>(
        common::instrument::data::GET_ALL_ACTIVATION_SNOOPS,
    );

    // Pull output/string batches
    let UseTauriWithReturn {
        trigger: fetch_output,
        data: output_batch,
        error: output_error,
        ..
    } = use_command::<common::instrument::data::StringSnoopBatchPayload>(
        common::instrument::data::GET_ALL_STRING_SNOOPS,
    );

    // Log errors (DRY: centralize logging here)
    Effect::new(move |_| {
        if let Some(err) = excitement_error() {
            log::error!(
                "Error invoking {}: {err}",
                common::instrument::data::GET_ALL_ACTIVATION_SNOOPS
            );
        }
        if let Some(err) = output_error() {
            log::error!(
                "Error invoking {}: {err}",
                common::instrument::data::GET_ALL_STRING_SNOOPS
            );
        }
    });

    // Drive periodic fetch
    let _raf = crate::util::raf_fn_fps::use_raf_fn_with_fps(
        move |_| {
            fetch_excitement(Some(()));
            fetch_output(Some(()));
        },
        REFRESH_FPS,
    );

    // Overlay geometry:
    // - top-left, non-interactive, tiny readable tiles
    // - grid: rows = groups, cols = keys-per-group
    let overlay_class = "absolute top-2 left-2 z-50";
    let grid_style = move || {
        let rows = num_groups();
        let cols = num_keys_per_group();
        format!(
            "display: grid; grid-template-rows: repeat({}, minmax(0,auto)); grid-template-columns: repeat({}, minmax(0,auto)); gap: 2px;",
            rows, cols
        )
    };

    view! {
        <div class=overlay_class>
            <div class="rounded bg-red/70 dark:bg-black/60 border border-black/20 dark:border-red/20 p-1 shadow-sm">
                <Fold title="Siren debug: Excitement vs Output">
                    <div style=grid_style class="text-[10px] pointer-events-none select-none">
                        {move || {
                            let ng = num_groups();
                            let nk = num_keys_per_group();
                            (0..ng as usize)
                                .flat_map(move |g| {
                                    (0..nk as usize)
                                        .map(move |k| {
                                            let act_samples = Signal::derive({
                                                move || {
                                                    excitement_batch()
                                                        .and_then(|b| {
                                                            b.snoops
                                                                .iter()
                                                                .find(|e| e.group as usize == g && e.key as usize == k)
                                                                .map(|e| e.samples.clone())
                                                        })
                                                }
                                            });
                                            let out_samples = Signal::derive({
                                                move || {
                                                    output_batch()
                                                        .and_then(|b| {
                                                            b.snoops
                                                                .iter()
                                                                .find(|e| e.group as usize == g && e.key as usize == k)
                                                                .map(|e| e.samples.clone())
                                                        })
                                                }
                                            });
                                            // Derive per-tile signals by pairing excitement/output entries

                                            view! { <PairTile g k act_samples out_samples /> }
                                        })
                                })
                                .collect_view()
                        }}
                    </div>
                </Fold>
            </div>
        </div>
    }
}

#[component]
fn PairTile(
    g: usize,
    k: usize,
    #[prop(into)] act_samples: Signal<Option<Vec<f32>>>,
    #[prop(into)] out_samples: Signal<Option<Vec<f32>>>,
) -> impl IntoView {
    // Simple, cheap metric = mean absolute value (clamped to [0,1])
    let act_level = Memo::new(move |_| mean_abs_clamped(act_samples.with(|s| s.clone())));
    let out_level = Memo::new(move |_| mean_abs_clamped(out_samples.with(|s| s.clone())));

    // Bar widths as percentages
    let act_w = move || format!("width: {:.0}%;", (act_level() * 100.0).clamp(0.0, 100.0));
    let out_w = move || format!("width: {:.0}%;", (out_level() * 100.0).clamp(0.0, 100.0));

    // Tile visual: id + two tiny bars (A/O)
    view! {
        <div class="pointer-events-none p-1 min-w-[70px] rounded bg-black/5 dark:bg-red/5 border border-black/10 dark:border-red/10">
            <div class="flex items-center justify-between gap-1 mb-[2px]">
                <span class="text-[9px] text-black/70 dark:text-red/70">
                    {move || format!("{g},{k}")}
                </span>
                <span class="text-[9px] tabular-nums text-black/50 dark:text-red/50">
                    {move || format!("{:.2}/{:.2}", act_level(), out_level())}
                </span>
            </div>

            <div class="mb-[2px]">
                <div class="h-[3px] w-[70px] bg-black/15 dark:bg-red/15 rounded">
                    <div class="h-[3px] bg-black dark:bg-red rounded" style=act_w></div>
                </div>
                <div class="text-[9px] text-black/50 dark:text-red/50 mt-[1px]">A</div>
            </div>

            <div>
                <div class="h-[3px] w-[70px] bg-black/15 dark:bg-red/15 rounded">
                    <div class="h-[3px] bg-black dark:bg-red rounded" style=out_w></div>
                </div>
                <div class="text-[9px] text-black/50 dark:text-red/50 mt-[1px]">O</div>
            </div>
        </div>
    }
}

fn mean_abs_clamped(samples: Option<Vec<f32>>) -> f32 {
    let mut v = 0.0f32;
    if let Some(s) = samples {
        if !s.is_empty() {
            // Mean absolute value; apply mild compression for visibility
            v = s.iter().map(|x| x.abs()).sum::<f32>() / (s.len() as f32);
            v = v.sqrt(); // perceptual-ish
        }
    }
    v.clamp(0.0, 1.0)
}
