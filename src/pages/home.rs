use leptos::prelude::*;

use crate::components::{ContentPage, Menu, UiPlacement};

#[component]
pub fn Home() -> impl IntoView {
    view! {
        <ContentPage
            title="Red Siren"
            no_back_button=true
            card_animation_direction=UiPlacement::Left
        >
            <Menu />
        </ContentPage>
    }
}
