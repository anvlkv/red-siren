use common::instrument::PlaybackQuality;
use leptos::prelude::*;
use tauri_use::{use_command, use_invoke, UseTauriReturn, UseTauriWithReturn};

use crate::{
    components::{Icon, Switch, UiPlacement, UiSize},
    util::raf_fn_fps::use_raf_fn_with_fps,
};

/// A segmented switch for selecting playback quality.
///
/// Displays five segments: Auto, LoFi, Medium, HiFi, Ultra.
/// When in Auto mode, the segment that the gate manager currently recommends
/// shows a small dot callout above its icon.
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

    let quality = Memo::new(move |prev| {
        quality_data().unwrap_or_else(|| prev.copied().unwrap_or_default())
    });

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

    // Map quality to switch segment index.
    let current_state = Signal::derive(move || match quality() {
        PlaybackQuality::Auto(_) => 0usize,
        PlaybackQuality::LoFi => 1,
        PlaybackQuality::Medium => 2,
        PlaybackQuality::HiFi => 3,
        PlaybackQuality::Ultra => 4,
    });

    // When in Auto mode, which segment is currently recommended.
    // None when the user has manually selected a quality.
    let recommended_index: Signal<Option<usize>> = Signal::derive(move || match quality() {
        PlaybackQuality::Auto(v) => Some(match v {
            i8::MIN..=-1 => 1usize, // LoFi
            0 => 2,                 // Medium
            1 => 3,                 // HiFi
            2..=i8::MAX => 4,       // Ultra
        }),
        _ => None,
    });

    let on_change = Callback::new(move |index: usize| {
        let new_quality = match index {
            0 => PlaybackQuality::Auto(0),
            1 => PlaybackQuality::LoFi,
            2 => PlaybackQuality::Medium,
            3 => PlaybackQuality::HiFi,
            _ => PlaybackQuality::Ultra,
        };
        set_quality_trigger(Some((
            common::instrument::commands::SetQualityPayload {
                quality: new_quality,
            },
            (),
        )));
    });

    // Build a reactive label for each segment.
    // The dot above the icon is only visible when this segment is the recommended one.
    let make_label = |segment_index: usize, icon_name: &'static str| {
        view! {
            <span class="relative inline-flex flex-col items-center gap-0.5">
                <span
                    class=move || {
                        if recommended_index() == Some(segment_index) {
                            "w-1 h-1 rounded-full bg-current"
                        } else {
                            "w-1 h-1 rounded-full opacity-0"
                        }
                    }
                />
                <Icon name=icon_name size=UiSize::Sm />
            </span>
        }
        .into_any()
    };

    view! {
        <Switch
            labels=vec![
                make_label(0, "system"),
                make_label(1, "squares"),
                make_label(2, "batch"),
                make_label(3, "cube"),
                make_label(4, "diamond"),
            ]
            tooltips=vec![
                "Auto".to_string(),
                "Lo-Fi".to_string(),
                "Medium".to_string(),
                "Hi-Fi".to_string(),
                "Ultra".to_string(),
            ]
            current_state=current_state
            on_change=on_change
            size=UiSize::Sm
            placement=placement
        />
    }
}
