use common::instrument::PlaybackQuality;
use leptos::prelude::*;
use tauri_use::{use_command, use_invoke, UseTauriReturn, UseTauriWithReturn};

use crate::{
    components::{Dropdown, Icon, UiPlacement, UiSize},
    util::raf_fn_fps::use_raf_fn_with_fps,
};

fn recommended_quality(value: i8) -> PlaybackQuality {
    match value {
        i8::MIN..=-1 => PlaybackQuality::LoFi,
        0 => PlaybackQuality::Medium,
        1 => PlaybackQuality::HiFi,
        2..=i8::MAX => PlaybackQuality::Ultra,
    }
}

fn quality_icon(quality: PlaybackQuality) -> &'static str {
    match quality {
        PlaybackQuality::Auto(v) => quality_icon(recommended_quality(v)),
        PlaybackQuality::LoFi => "squares",
        PlaybackQuality::Medium => "batch",
        PlaybackQuality::HiFi => "cube",
        PlaybackQuality::Ultra => "diamond",
    }
}

fn dropdown_arrow(placement: UiPlacement) -> &'static str {
    match placement {
        UiPlacement::Top => "▲",
        UiPlacement::Bottom => "▼",
        UiPlacement::Left => "◂",
        UiPlacement::Right => "▸",
    }
}

/// A segmented switch for selecting playback quality.
///
/// Displays two segments: auto and current.
/// Selecting current opens a dropdown with explicit quality options.
#[component]
pub fn PlayBackQualitySwitch(
    /// Drives orientation: Left/Right → vertical, Top/Bottom → horizontal.
    #[prop(into, optional)]
    placement: Signal<Option<UiPlacement>>,
) -> impl IntoView {
    let UseTauriWithReturn {
        trigger: check_quality,
        data: quality_data,
        error: quality_error,
    } = use_command::<PlaybackQuality>(common::instrument::commands::QUALITY_INDICATOR);

    let quality =
        Memo::new(move |prev| quality_data().unwrap_or_else(|| prev.copied().unwrap_or_default()));

    _ = use_raf_fn_with_fps(
        move |_| {
            check_quality(Some(()));
        },
        20.0,
    );

    let UseTauriReturn {
        trigger: set_quality_trigger,
        error: set_quality_error,
        ..
    } = use_invoke::<common::instrument::commands::SetQualityPayload, (), ()>(
        common::instrument::commands::SET_QUALITY,
    );

    Effect::new(move |_| {
        if let Some(err) = quality_error() {
            log::error!(
                "Error checking {}: {err}",
                common::instrument::commands::QUALITY_INDICATOR
            );
        }
        if let Some(err) = set_quality_error() {
            log::error!(
                "Error invoking {}: {err}",
                common::instrument::commands::SET_QUALITY
            );
        }
    });

    let auto_selected = Signal::derive(move || matches!(quality(), PlaybackQuality::Auto(_)));

    let current_quality_icon = Signal::derive(move || match quality() {
        PlaybackQuality::Auto(v) => quality_icon(recommended_quality(v)).to_string(),
        explicit => quality_icon(explicit).to_string(),
    });

    let dropdown_placement = Signal::derive(move || {
        placement()
            .map(|current| current.opposite())
            .or(Some(UiPlacement::Top))
    });

    let arrow = Signal::derive(move || {
        dropdown_arrow(dropdown_placement().unwrap_or(UiPlacement::Top)).to_string()
    });

    let select_quality = move |new_quality: PlaybackQuality| {
        set_quality_trigger(Some((
            common::instrument::commands::SetQualityPayload {
                quality: new_quality,
            },
            (),
        )));
    };

    view! {
        <div class="inline-flex overflow-visible rounded-lg border-2 border-black dark:border-red">
            <button
                type="button"
                class=move || {
                    if auto_selected() {
                        "relative flex items-center justify-center md:h-10 h-8 md:px-4 px-2 md:text-base text-sm bg-black dark:bg-red text-red dark:text-black shadow-inner"
                    } else {
                        "relative flex items-center justify-center md:h-10 h-8 md:px-4 px-2 md:text-base text-sm text-black dark:text-red hover:bg-black/5 dark:hover:bg-red/5"
                    }
                }
                on:click=move |_| {
                    set_quality_trigger(
                        Some((
                            common::instrument::commands::SetQualityPayload {
                                quality: PlaybackQuality::Auto(0),
                            },
                            (),
                        )),
                    );
                }
                aria-pressed=auto_selected
                title="Auto mode"
            >
                <span class="inline-flex items-center justify-center" aria-hidden="true">
                    <Icon name="system" size=UiSize::Sm />
                </span>
            </button>

            <Dropdown
                placement=dropdown_placement
                class="min-w-32"
                trigger_class=Signal::derive(move || {
                    if auto_selected() {
                        "border-l border-black/20 dark:border-red/20 md:h-10 h-8 md:px-4 px-2 md:text-base text-sm text-black dark:text-red hover:bg-black/5 dark:hover:bg-red/5"
                            .to_string()
                    } else {
                        "md:h-10 h-8 md:px-4 px-2 md:text-base text-sm bg-black dark:bg-red text-red dark:text-black shadow-inner"
                            .to_string()
                    }
                })
                trigger=view! {
                    <span class="inline-flex items-center justify-center gap-1" aria-hidden="true">
                        <Icon name=current_quality_icon size=UiSize::Sm />
                        <span class="text-base leading-none opacity-80 font-semibold">{arrow}</span>
                    </span>
                }
                    .into_any()
            >
                <div class="flex flex-col gap-1">
                    {[
                        ("lo-fi", PlaybackQuality::LoFi),
                        ("medium", PlaybackQuality::Medium),
                        ("hi-fi", PlaybackQuality::HiFi),
                        ("ultra", PlaybackQuality::Ultra),
                    ]
                        .into_iter()
                        .map(|(label, item_quality)| {
                            let is_selected = Signal::derive(move || quality() == item_quality);
                            let icon_name = quality_icon(item_quality);

                            view! {
                                <button
                                    type="button"
                                    role="menuitemradio"
                                    aria-checked=is_selected
                                    class=move || {
                                        if is_selected() {
                                            "w-full rounded-md px-3 py-1 text-left bg-black text-red dark:bg-red dark:text-black"
                                        } else {
                                            "w-full rounded-md px-3 py-1 text-left hover:bg-black/10 dark:hover:bg-red/20"
                                        }
                                    }
                                    on:click=move |_| {
                                        select_quality(item_quality);
                                    }
                                >
                                    <span class="inline-flex w-full items-center justify-between gap-2">
                                        <span class="inline-flex items-center gap-2">
                                            <Icon name=icon_name size=UiSize::Sm />
                                            <span class="leading-none">{label}</span>
                                        </span>
                                        <Show when=is_selected>
                                            <Icon name="ok" size=UiSize::Sm class="opacity-80" />
                                        </Show>
                                    </span>
                                </button>
                            }
                        })
                        .collect_view()}
                </div>
            </Dropdown>
        </div>
    }
}
