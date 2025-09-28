use crate::components::{Icon, Switch};
use leptos::prelude::*;

#[component]
pub fn Play() -> impl IntoView {
    let activation_source = RwSignal::new(0);

    view! {
        <Switch
            labels=vec![
                view! { <Icon name="mic" /> }.into_any(),
                view! { <Icon name="entropy" /> }.into_any(),
            ]
            current_state=activation_source
            on_change=Callback::new(|s| log::info!("toggled to : {s}"))
        />
    }
}
