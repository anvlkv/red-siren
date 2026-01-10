use crate::{
    components::{
        expect_tuner_service, Button, CompactMenu, Icon, MenuItem, Tuner, UiPlacement, UiSize,
        UiVariant,
    },
    util::{
        layout_context::{expect_layout_context, LayoutContextReturn},
        secondary_window::is_secondary_window,
        setup_context::is_mic_premission_granted,
    },
};
use common::RouteId;
use leptos::prelude::*;
use leptos_router::hooks::use_navigate;

#[component]
pub fn Tune() -> impl IntoView {
    // Access long-lived tuner service (commands + shared state)
    let tuner_service = expect_tuner_service();

    let is_secondary_window = is_secondary_window();

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

    let LayoutContextReturn { orientation, .. } = expect_layout_context();

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
        }
    });

    // Probe handler
    let on_probe = Callback::new({
        let tuner_service = tuner_service.clone();
        move |_: ()| {
            tuner_service.probe.run(());
        }
    });

    let title = Signal::derive(move || {
        if is_secondary_window() {
            Some("Tune")
        } else {
            None
        }
    });

    // Get navigation function
    let navigate = use_navigate();

    // Check mic permission on mount and redirect if needed
    let mic_permission = is_mic_premission_granted();
    Effect::new({
        let navigate = navigate.clone();
        move |_| {
            if mic_permission() != Some(true) {
                log::info!("Mic permission required for tuner, redirecting to Permissions");
                navigate(RouteId::Permissions.as_ref(), Default::default());
            }
        }
    });

    view! {
        <div class="relative w-full h-full">
            <Tuner />
            <CompactMenu items=menu_items placement hide_home=is_secondary_window title>
                <Button
                    on:click=move |_| on_reset.run(())
                    size=UiSize::Sm
                    variant=UiVariant::Outline
                    placement=placement
                    attr:r#type="reset"
                >
                    <Icon name="reset" size=UiSize::Sm />
                    <span class="inline-block flex-grow text-center">"Reset"</span>
                </Button>
                <Button
                    on:click=move |_| on_probe.run(())
                    size=UiSize::Sm
                    variant=Signal::derive(move || {
                        if !tuner_service.probe_active.get() {
                            UiVariant::Outline
                        } else {
                            UiVariant::Solid
                        }
                    })
                    placement=placement
                >
                    <Icon name="probe" size=UiSize::Sm />
                    <span class="inline-block flex-grow text-center">
                        {move || {
                            if !tuner_service.probe_active.get() {
                                "Probe audio"
                            } else {
                                "Stop probing"
                            }
                        }}
                    </span>
                </Button>
            </CompactMenu>
        </div>
    }
}
