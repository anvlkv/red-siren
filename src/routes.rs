use leptos::prelude::*;
use leptos_router::NavigateOptions;
use leptos_router::{components::*, hooks::use_navigate, path};
use tauri_use::{use_listen, EventType, UseListenReturn};

use crate::components::{AppError, ErrorTemplate};
use crate::pages::Home;

#[component]
pub fn AppRoutes() -> impl IntoView {
    let UseListenReturn {
        data: destination, ..
    } = use_listen::<String>(EventType::Custom(shared::events::navigation::GO_TO));
    let navigate = use_navigate();

    Effect::new(move |_| {
        if let Some(dest) = destination().as_ref() {
            navigate(dest, NavigateOptions::default());
        }
    });

    view! {
        <Routes fallback=|| {
            let mut outside_errors = Errors::default();
            outside_errors.insert_with_default_key(AppError::NotFound);
            view! { <ErrorTemplate outside_errors /> }.into_view()
        }>
            <Route path=path!("/") view=move || view! { <Home /> } />
        // <Route path="about" view=move || view! { <about::AboutComponent /> } />
        // <Route path="play" view=move || view! { <instrument::InstrumentComponent /> } />
        // <Route path="tune" view=move || view! { <tuner::TunerComponent /> } />
        </Routes>
    }
}
