use leptos::prelude::*;
use shared::{
    instrument::events::{ActivationSourcePayload, PlaybackStatePayload},
    RouteId,
};

use crate::{
    components::{CompactMenu, Icon, MenuItem, Switch, UiSize},
    util::tauri_resource::{use_tauri_resource, UseTauriResourceReturn},
};

#[component]
pub fn Play() -> impl IntoView {
    let UseTauriResourceReturn {
        data: activation_source,
        ..
    } = use_tauri_resource::<ActivationSourcePayload>(shared::instrument::events::ACTIVATION_SRC);

    let UseTauriResourceReturn { data: playback, .. } =
        use_tauri_resource::<PlaybackStatePayload>(shared::instrument::events::PLAYBACK_STATE);

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
        if let Some(PlaybackStatePayload { playing }) = playback() {
            if playing {
                set_menu_items.update(|m| {
                    m[0] = MenuItem::Action {
                        icon: "pause",
                        label: "Pause",
                        action: Callback::new(|_| {}),
                    }
                });
            } else {
                set_menu_items.update(|m| {
                    m[0] = MenuItem::Action {
                        icon: "resume",
                        label: "Play",
                        action: Callback::new(|_| {}),
                    }
                });
            }
        }
    });

    view! {
        <div class="relative h-full w-full flex flex-col items-center justify-end">
            <CompactMenu items=menu_items>
                <Switch
                    labels=vec![
                        view! { <Icon name="entropy" /> }.into_any(),
                        view! { <Icon name="mic" /> }.into_any(),
                    ]
                    tooltips=vec!["Random".to_string(), "Mic".to_string()]
                    current_state=Signal::derive(move || {
                        activation_source().map(|s| s.source).unwrap_or_default() as usize
                    })
                    on_change=Callback::new(|s| log::info!("toggled to : {s}"))
                    size=UiSize::Sm
                />
            </CompactMenu>
        </div>
    }
}
