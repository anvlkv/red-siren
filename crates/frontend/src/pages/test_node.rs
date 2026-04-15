use crate::components::{ContentPage, TestNodePanel, UiPlacement};
use leptos::prelude::*;

#[component]
pub fn TestNode() -> impl IntoView {
    view! {
        <ContentPage
            title=common::RouteId::TestNode.title()
            card_animation_direction=UiPlacement::Left
        >
            <TestNodePanel />
        </ContentPage>
    }
}
