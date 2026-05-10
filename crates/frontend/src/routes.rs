use leptos::prelude::*;
use leptos_router::{components::*, path};

use crate::pages::Home;

#[component]
pub fn AppRoutes() -> impl IntoView {
    view! {
        <Routes fallback=|| view! { <Home /> }>
            <Route path=path!("/") view=Home />
        </Routes>
    }
}
