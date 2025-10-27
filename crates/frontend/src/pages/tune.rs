use crate::{
    components::{
        expect_tuner_service, Button, CompactMenu, Icon, MenuItem, Tuner, UiPlacement, UiSize,
        UiVariant,
    },
    util::{
        layout_context::{expect_layout_contex, LayoutContextReturn},
        secondary_window::is_secondary_window,
        tauri_resource::{use_tauri_resource, UseTauriResourceReturn},
    },
};
use common::RouteId;
use leptos::prelude::*;

#[component]
pub fn Tune() -> impl IntoView {
    // Access long-lived tuner service (commands + shared state)
    let tuner_service = expect_tuner_service();

    let is_secondary_window = is_secondary_window();

    // Tuner config resource (refetch after reset)
    let UseTauriResourceReturn { refetch, .. } =
        use_tauri_resource::<common::tuner::Config>(common::commands::tuner::CONFIG);

    let menu_items = Memo::new(move |_| {
        if is_secondary_window() {
            vec![]
        } else {
            vec![
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
            ]
        }
    });

    let LayoutContextReturn { orientation, .. } = expect_layout_contex();

    let placement = Signal::derive(move || match orientation() {
        common::orientation::LayoutOrientation::Vertical => UiPlacement::Left,
        common::orientation::LayoutOrientation::Horizontal => UiPlacement::Bottom,
    });

    // Start tuner stream on mount
    Effect::new({
        let tuner_service = tuner_service.clone();
        move |_| {
            tuner_service.start_stream.run(());
        }
    });

    // Stop tuner stream on unmount
    on_cleanup({
        let tuner_service = tuner_service.clone();
        move || tuner_service.stop_stream.run(())
    });

    // Reset handler
    let on_reset = Callback::new({
        let tuner_service = tuner_service.clone();
        move |_: ()| {
            tuner_service.reset.run(());
            refetch(); // Refresh config after reset
        }
    });

    view! {
        <div class="relative w-full h-full">
            <Tuner />
            <CompactMenu items=menu_items placement=placement>
                <Button
                    on:click=move |_| on_reset.run(())
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
