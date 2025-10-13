use crate::{
    components::{
        Button, CompactMenu, Icon, MenuItem, TunerWithContext, UiPlacement, UiSize, UiVariant,
    },
    util::{
        layout_context::{expect_layout_contex, LayoutContextReturn},
        tauri_resource::{use_tauri_resource, UseTauriResourceReturn},
    },
};
use common::RouteId;
use leptos::prelude::*;
use tauri_use::{use_invoke, UseTauriReturn};

#[component]
pub fn Tune() -> impl IntoView {
    // Tuner config resource
    let UseTauriResourceReturn { refetch, .. } =
        use_tauri_resource::<common::tuner::Config>(common::commands::tuner::CONFIG);

    // Reset command
    let UseTauriReturn {
        error: reset_error,
        trigger: reset_invoke,
        ..
    } = use_invoke::<(), (), ()>(common::commands::tuner::RESET_CONFIG);

    let (menu_items, _set_menu_items) = signal(vec![
        MenuItem::Navigate {
            route: RouteId::Play,
            icon: "play",
            label: "Play",
        },
        MenuItem::Navigate {
            route: RouteId::About,
            icon: "info",
            label: "About",
        },
    ]);

    let LayoutContextReturn { orientation, .. } = expect_layout_contex();

    let placement = Signal::derive(move || match orientation() {
        common::orientation::LayoutOrientation::Vertical => UiPlacement::Left,
        common::orientation::LayoutOrientation::Horizontal => UiPlacement::Bottom,
    });

    Effect::new(move |_| {
        if let Some(err) = reset_error() {
            log::error!(
                "Error invoking {}: {err}",
                common::commands::tuner::RESET_CONFIG
            );
        }
    });

    let on_reset = move |_| {
        reset_invoke(Some(((), ())));
        refetch(); // Refresh config after reset
    };

    view! {
        <div class="relative w-full h-full">
            <TunerWithContext />
            <CompactMenu items=menu_items placement=placement>
                <Button
                    on:click=on_reset
                    size=UiSize::Sm
                    variant=UiVariant::Outline
                    placement=placement
                    attr:r#type="reset"
                >
                    <Icon name="reset" size=UiSize::Sm />
                    <span class="inline-block flex-grow text-center">Reset</span>
                </Button>
            </CompactMenu>
        </div>
    }
}
