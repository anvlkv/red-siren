use common::{instrument::PlaybackQuality, RouteId};
use leptos::prelude::*;
use leptos_router::hooks::use_navigate;
use tauri_use::{use_command, UseTauriWithReturn};

use crate::{
    components::{
        AppearanceToggle, CompactMenu, ExcitementSourceToggle, Icon, Instrument, MenuItem,
        UiPlacement, UiSize,
    },
    util::{
        layout_context::{expect_layout_context, LayoutContextReturn},
        playback_service::{expect_playback_service, PlaybackService},
        raf_fn_fps::use_raf_fn_with_fps,
        secondary_window::is_secondary_window,
        setup_context::{initial_mic_permission, is_devtools_enabled},
        tauri_resource::{use_tauri_resource, UseTauriResourceReturn},
    },
};

#[component]
pub fn Play() -> impl IntoView {
    let UseTauriResourceReturn {
        data: playback_state,
        ..
    } = use_tauri_resource::<common::instrument::events::PlaybackStatePayload>(
        common::instrument::events::PLAYBACK_STATE,
    );

    let is_secondary_window = is_secondary_window();

    // Get navigation function
    let navigate = use_navigate();

    // Get setup state to check mic permission
    let mic_permission = initial_mic_permission();

    // Check mic permission on mount and redirect if needed
    Effect::new({
        let navigate = navigate.clone();
        move |_| {
            if mic_permission().is_none() {
                log::info!("Mic permission not set, redirecting to Permissions");
                navigate(RouteId::Permissions.as_ref(), Default::default());
            }
        }
    });

    // Derive compact menu placement from current instrument layout orientation
    let LayoutContextReturn { orientation, .. } = expect_layout_context();

    let placement = Signal::derive(move || match orientation() {
        common::orientation::LayoutOrientation::Vertical => UiPlacement::Left,
        common::orientation::LayoutOrientation::Horizontal => UiPlacement::Bottom,
    });

    let PlaybackService {
        start: cb_start,
        pause: cb_pause,
        resume: cb_resume,
        stop: cb_stop,
    } = expect_playback_service();

    let (menu_items, set_menu_items) = signal(vec![
        MenuItem::Action {
            icon: "pause",
            label: "Pause",
            action: Callback::new(|_| {}),
        },
        MenuItem::Navigate {
            route: RouteId::Tune,
            icon: "tune",
            label: "Tune",
        },
        MenuItem::Navigate {
            route: RouteId::About,
            icon: "info",
            label: "About",
        },
    ]);

    Effect::new(move |_| {
        if let Some(common::instrument::events::PlaybackStatePayload { playing }) = playback_state()
        {
            log::debug!("Updating menu items, the playback is [{playing}]");

            if playing {
                set_menu_items.update(|m| {
                    m[0] = MenuItem::Action {
                        icon: "pause",
                        label: "Pause",
                        action: Callback::new({
                            move |_| {
                                cb_pause.run(());
                            }
                        }),
                    }
                });
            } else {
                set_menu_items.update(|m| {
                    m[0] = MenuItem::Action {
                        icon: "resume",
                        label: "Play",
                        action: Callback::new(move |_| cb_resume.run(())),
                    }
                });
            }
        }
    });

    Effect::new(move |_| {
        cb_start.run(());
    });

    on_cleanup(move || {
        cb_stop.run(());
    });

    let editor = is_devtools_enabled();

    let icon_ref = NodeRef::new();

    let UseTauriWithReturn {
        trigger: check_quality_indicator,
        data: quality_indicator_data,
        error: quality_indicator_error,
    } = use_command::<PlaybackQuality>(common::instrument::commands::QUALITY_INDICATOR);

    let quality_indicator_data = Memo::new(move |prev| {
        quality_indicator_data().unwrap_or_else(|| prev.copied().unwrap_or_default())
    });

    _ = use_raf_fn_with_fps(
        move |_| {
            check_quality_indicator(Some(()));
        },
        20.0,
    );

    Effect::new(move |_| {
        if let Some(err) = quality_indicator_error() {
            log::error!("Error checking quality status: {}", err);
        }
    });

    // with_tooltip(
    //     icon_ref,
    //     Signal::derive(move || {
    //         match quality_indicator_data() {
    //             PlaybackQuality::HighQuality => "High Quality",
    //             PlaybackQuality::OptimizedQuality => "Optimized",
    //             PlaybackQuality::Resetting => "Restarting...",
    //             PlaybackQuality::Underruns => "Overload!",
    //         }
    //         .to_string()
    //     }),
    //     Signal::derive(move || Some(placement().opposite())),
    // );

    let process_icon = Signal::derive(move || match quality_indicator_data() {
        PlaybackQuality::Auto(_) => "robot",
        PlaybackQuality::Ultra => "diamond",
        PlaybackQuality::HiFi => "cube",
        PlaybackQuality::Medium => "batch",
        PlaybackQuality::LoFi => "squares",
    });

    view! {
        <div>
            <Instrument editor />
            <Show when=move || !is_secondary_window()>
                <CompactMenu items=menu_items placement>
                    <Icon
                        name=process_icon
                        size=UiSize::Sm
                        class="text-gary dark:text-cinnabar"
                        node_ref=icon_ref
                    />
                    <ExcitementSourceToggle placement mic_permission />
                    <AppearanceToggle placement />
                </CompactMenu>
            </Show>
        </div>
    }
}
