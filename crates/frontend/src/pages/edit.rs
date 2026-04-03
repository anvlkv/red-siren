use std::str::FromStr;

use common::{EditorRouteId, RouteId};
use leptos::prelude::*;
use leptos_router::{
    hooks::use_location, hooks::use_navigate, location::Location, NavigateOptions,
};

use crate::{
    components::{ContentPage, EditorPanel, LayoutEditorPanel, RoutedTab, RoutedTabs},
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

    let tabs = Signal::derive(move || {
        vec![
            RoutedTab {
                label: "Layout",
                route: RouteId::Edit(EditorRouteId::Layout),
            },
            RoutedTab {
                label: "Fine-tuned values",
                route: RouteId::Edit(EditorRouteId::FinetunedValues),
            },
        ]
    });

    let Location { pathname, .. } = use_location();
    let title = Signal::derive(move || {
        RouteId::from_str(&pathname())
            .unwrap_or(RouteId::Edit(EditorRouteId::Layout))
            .title()
            .to_string()
    });

    view! {
        <ContentPage title=title>
            <RoutedTabs tabs=tabs class="w-auto max-w-full md:w-md xl:w-xl 3xl:w-2xl  h-lvh" />
        </ContentPage>
    }
}

#[component]
pub fn EditLayout() -> impl IntoView {
    view! { <LayoutEditorPanel /> }
}

#[component]
pub fn EditFineTunedValues() -> impl IntoView {
    view! { <EditorPanel /> }
}
