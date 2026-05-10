use leptos::prelude::*;
use leptos_router::components::Router;

use crate::routes::AppRoutes;

#[component]
pub fn App() -> impl IntoView {
    view! {
        <>
            <leptos_styling::StyleSheets />
            <main class="reset-shell font-serif selection:bg-black/15">
                <Router>
                    <AppRoutes />
                </Router>
            </main>
        </>
    }
}
