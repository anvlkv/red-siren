use common::RouteId;
use leptos::prelude::*;
use leptos_router::components::*;
use leptos_router::StaticSegment;

use crate::{
    components::{AppError, ErrorTemplate, Intro},
    pages::{About, Donate, Home, Permissions, Play, Tune},
};

#[component]
pub fn AppRoutes() -> impl IntoView {
    view! {
        <Intro nav_tx=nav_tx_signal nav_started=started current_route=current_route>
            <div class="absolute h-full w-full overflow-hidden pointer-events-auto z-1" role="main">
                <Routes
                    fallback=|| {
                        let mut outside_errors = Errors::default();
                        outside_errors.insert_with_default_key(AppError::NotFound);
                        view! { <ErrorTemplate outside_errors /> }.into_view()
                    }
                    transition=true
                >
                    <Route path=(StaticSegment(RouteId::Home.as_ref()),) view=Home />
                    <Route path=(StaticSegment(RouteId::Play.as_ref()),) view=Play />
                    <Route path=(StaticSegment(RouteId::Tune.as_ref()),) view=Tune />
                    <Route path=(StaticSegment(RouteId::About.as_ref()),) view=About />
                    <Route path=(StaticSegment(RouteId::Donate.as_ref()),) view=Donate />
                    <Route path=(StaticSegment(RouteId::Permissions.as_ref()),) view=Permissions />
                </Routes>
            </div>
        </Intro>
    }
}
