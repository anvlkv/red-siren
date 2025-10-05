use common::RouteId;
use leptos::prelude::*;

use crate::{
    components::{
        ActivationSourceToggle, AppearanceToggle, CompactMenu, Instrument, MenuItem, UiPlacement,
    },
    util::{
        layout_context::{expect_layout_contex, LayoutContextReturn},
        playback_service::{expect_playback_service, PlaybackService},
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

    // Derive compact menu placement from current instrument layout orientation
    let LayoutContextReturn { orientation, .. } = expect_layout_contex();

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
