use crate::components::{CompactMenu, Icon, MenuItem, Switch};
use leptos::prelude::*;
use shared::RouteId;

#[component]
pub fn Play() -> impl IntoView {
    let activation_source = RwSignal::new(0);

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

    view! {
        <CompactMenu items=menu_items>
            <Switch
                labels=vec![
                    view! { <Icon name="mic" /> }.into_any(),
                    view! { <Icon name="entropy" /> }.into_any(),
                ]
                current_state=activation_source
                on_change=Callback::new(|s| log::info!("toggled to : {s}"))
            />
        </CompactMenu>
    }
}
