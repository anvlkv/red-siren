use std::str::FromStr;

use common::{EditorRouteId, RouteId};
use leptos::prelude::*;
use leptos_router::{
    hooks::use_location, hooks::use_navigate, location::Location, NavigateOptions,
};

use crate::{
    components::{ContentPage, RoutedTab, RoutedTabs},
    util::{boot_flags::boot_flags, playback_service::expect_playback_service},
};

#[component]
pub fn Edit() -> impl IntoView {
    let service = expect_playback_service();

    Effect::new(move |_| {
        service.start.run(());
    });

    on_cleanup(move || {
        service.stop.run(());
    });

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
                route: RouteId::Edit(EditorRouteId::InstrumentLayout),
            },
            RoutedTab {
                label: "Fine-tuned values",
                route: RouteId::Edit(EditorRouteId::FinetunedValues),
            },
            RoutedTab {
                label: "Rhythm grid",
                route: RouteId::Edit(EditorRouteId::RhythmGrid),
            },
            RoutedTab {
                label: "Node test bed",
                route: RouteId::Edit(EditorRouteId::NodeTestBed),
            },
        ]
    });

    let Location { pathname, .. } = use_location();
    let title = Signal::derive(move || {
        RouteId::from_str(&pathname())
            .unwrap_or(RouteId::Edit(EditorRouteId::InstrumentLayout))
            .title()
            .to_string()
    });

    view! {
        <ContentPage title=title>
            <RoutedTabs tabs=tabs class="w-auto max-w-full h-lvh" />
        </ContentPage>
    }
}
