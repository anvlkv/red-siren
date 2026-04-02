use common::RouteId;
use leptos::prelude::*;
use leptos_router::{hooks::use_navigate, NavigateOptions};

use crate::{
    components::{ContentPage, EditorPanel},
    util::boot_flags::boot_flags,
};

#[component]
pub fn Edit() -> impl IntoView {
    let navigate = use_navigate();
    let devtools_enabled = boot_flags().devtools;

    Effect::new({
        let navigate = navigate.clone();
        move |_| {
            if !devtools_enabled {
                navigate(
                    RouteId::Home.as_ref(),
                    NavigateOptions {
                        replace: true,
                        ..Default::default()
                    },
                );
            }
        }
    });

    view! {
        <ContentPage title=RouteId::Edit.title().to_string()>
            <EditorPanel />
        </ContentPage>
    }
}
