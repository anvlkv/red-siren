use crate::components::{ContentPage, TestNodePanel, UiPlacement};
use crate::util::playback_service::expect_playback_service;
use leptos::prelude::*;

#[component]
pub fn TestNode() -> impl IntoView {
    let service = expect_playback_service();

    Effect::new(move |_| {
        service.start.run(());
    });

    on_cleanup(move || {
        service.stop.run(());
    });

    view! {
        <ContentPage
            title=common::RouteId::TestNode.title()
            card_animation_direction=UiPlacement::Left
        >
            <TestNodePanel />
        </ContentPage>
    }
}
