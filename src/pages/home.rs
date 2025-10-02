use leptos::prelude::*;
use shared::RouteId;

use crate::components::{ContentPage, Menu};

#[component]
pub fn Home() -> impl IntoView {
    view! {
        <ContentPage route_id=RouteId::Home title="Red Siren" no_back_button=true>
            <Menu />
        </ContentPage>
    }
}
