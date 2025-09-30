use leptos::prelude::*;
use shared::RouteId;
use tauri_use::{use_command, use_invoke, UseTauriReturn, UseTauriWithReturn};

use crate::{
    components::{CompactMenu, Icon, MenuItem, Switch, UiSize},
    util::tauri_resource::{use_tauri_resource, UseTauriResourceReturn},
};

#[component]
pub fn Play() -> impl IntoView {
    let UseTauriResourceReturn {
        data: activation_source,
        ..
    } = use_tauri_resource::<shared::instrument::events::ActivationSourcePayload>(
        shared::instrument::events::ACTIVATION_SRC,
    );

    let UseTauriResourceReturn { data: playback, .. } =
        use_tauri_resource::<shared::instrument::events::PlaybackStatePayload>(
            shared::instrument::events::PLAYBACK_STATE,
        );

    let UseTauriWithReturn {
        error: pause_error,
        trigger: trigger_pause,
        ..
    } = use_command::<()>(shared::instrument::commands::PLAYBACK_PAUSE);

    let UseTauriWithReturn {
        error: resume_error,
        trigger: trigger_resume,
        ..
    } = use_command::<()>(shared::instrument::commands::PLAYBACK_RESUME);

    let UseTauriWithReturn {
        error: start_error,
        trigger: trigger_start,
        ..
    } = use_command::<()>(shared::instrument::commands::PLAYBACK_START);

    let UseTauriWithReturn {
        error: stop_error,
        trigger: trigger_stop,
        ..
    } = use_command::<()>(shared::instrument::commands::PLAYBACK_STOP);

    let UseTauriReturn {
        error: set_activation_source_error,
        trigger: trigger_set_activation_source,
        ..
    } = use_invoke::<shared::commands::instrument::ActivationSourcePayload, (), ()>(
        shared::instrument::commands::SET_ACTIVATION_SRC,
    );

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

    let on_activation_source_change = Callback::new(move |source: usize| {
        trigger_set_activation_source(Some((
            shared::instrument::commands::ActivationSourcePayload {
                source: source as u8,
            },
            (),
        )));
    });

    Effect::new(move |_| {
        if let Some(err) = pause_error() {
            log::error!(
                "Error invoking {}: {err}",
                shared::instrument::commands::PLAYBACK_PAUSE
            );
        }

        if let Some(err) = resume_error() {
            log::error!(
                "Error invoking {}: {err}",
                shared::instrument::commands::PLAYBACK_RESUME
            );
        }

        if let Some(err) = start_error() {
            log::error!(
                "Error invoking {}: {err}",
                shared::instrument::commands::PLAYBACK_START
            );
        }

        if let Some(err) = stop_error() {
            log::error!(
                "Error invoking {}: {err}",
                shared::instrument::commands::PLAYBACK_STOP
            );
        }

        if let Some(err) = set_activation_source_error() {
            log::error!(
                "Error invoking {}: {err}",
                shared::instrument::commands::SET_ACTIVATION_SRC
            );
        }
    });

    Effect::new(move |_| {
        if let Some(shared::instrument::events::PlaybackStatePayload { playing }) = playback() {
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
        <div class="relative h-full w-full flex flex-col items-center justify-end">
            <CompactMenu items=menu_items>
                <Switch
                    labels=vec![
                        view! { <Icon name="entropy" size=UiSize::Sm /> }.into_any(),
                        view! { <Icon name="mic" size=UiSize::Sm /> }.into_any(),
                    ]
                    tooltips=vec!["Random".to_string(), "Mic".to_string()]
                    current_state=Signal::derive(move || {
                        activation_source().map(|s| s.source).unwrap_or_default() as usize
                    })
                    on_change=on_activation_source_change
                    size=UiSize::Sm
                />
            </CompactMenu>
        </div>
    }
}
