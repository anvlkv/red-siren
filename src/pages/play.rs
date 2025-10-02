use leptos::prelude::*;
use common::RouteId;
use tauri_use::{
    use_command, use_invoke, use_invoke_with_args, UseTauriReturn, UseTauriWithReturn,
};

use crate::{
    components::{
        ActivationSourceToggle, AppearanceToggle, CompactMenu, Icon, Instrument, MenuItem, Switch,
        UiPlacement, UiSize,
    },
    util::{
        layout_context::{expect_layout_contex, LayoutContextReturn},
        tauri_resource::{use_tauri_resource, UseTauriResourceReturn},
    },
};

#[component]
pub fn Play() -> impl IntoView {
    let UseTauriResourceReturn { data: playback, .. } =
        use_tauri_resource::<common::instrument::events::PlaybackStatePayload>(
            common::instrument::events::PLAYBACK_STATE,
        );

    // Derive compact menu placement from current instrument layout orientation
    let LayoutContextReturn { orientation, .. } = expect_layout_contex();

    let placement = Signal::derive(move || match orientation() {
        common::orientation::LayoutOrientation::Vertical => UiPlacement::Left,
        common::orientation::LayoutOrientation::Horizontal => UiPlacement::Bottom,
    });

    let UseTauriWithReturn {
        error: pause_error,
        trigger: trigger_pause,
        ..
    } = use_command::<()>(common::instrument::commands::PLAYBACK_PAUSE);

    let UseTauriWithReturn {
        error: resume_error,
        trigger: trigger_resume,
        ..
    } = use_command::<()>(common::instrument::commands::PLAYBACK_RESUME);

    let UseTauriWithReturn {
        error: start_error,
        trigger: trigger_start,
        ..
    } = use_command::<()>(common::instrument::commands::PLAYBACK_START);

    let UseTauriWithReturn {
        error: stop_error,
        trigger: trigger_stop,
        ..
    } = use_command::<()>(common::instrument::commands::PLAYBACK_STOP);

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
        if let Some(err) = pause_error() {
            log::error!(
                "Error invoking {}: {err}",
                common::instrument::commands::PLAYBACK_PAUSE
            );
        }

        if let Some(err) = resume_error() {
            log::error!(
                "Error invoking {}: {err}",
                common::instrument::commands::PLAYBACK_RESUME
            );
        }

        if let Some(err) = start_error() {
            log::error!(
                "Error invoking {}: {err}",
                common::instrument::commands::PLAYBACK_START
            );
        }

        if let Some(err) = stop_error() {
            log::error!(
                "Error invoking {}: {err}",
                common::instrument::commands::PLAYBACK_STOP
            );
        }
    });

    Effect::new(move |_| {
        if let Some(common::instrument::events::PlaybackStatePayload { playing }) = playback() {
            log::debug!("Updating menu items, the playback is [{playing}]");

            if playing {
                set_menu_items.update(|m| {
                    m[0] = MenuItem::Action {
                        icon: "pause",
                        label: "Pause",
                        action: Callback::new(move |_| {
                            trigger_pause(Some(()));
                        }),
                    }
                });
            } else {
                set_menu_items.update(|m| {
                    m[0] = MenuItem::Action {
                        icon: "resume",
                        label: "Play",
                        action: Callback::new(move |_| trigger_resume(Some(()))),
                    }
                });
            }
        }
    });

    Effect::new(move |_| {
        trigger_start(Some(()));
    });

    on_cleanup(move || {
        trigger_stop(Some(()));
    });

    view! {
        <div>
            <Instrument />
            <CompactMenu items=menu_items placement>
                <ActivationSourceToggle placement />
                <AppearanceToggle placement />
            </CompactMenu>
        </div>
    }
}
