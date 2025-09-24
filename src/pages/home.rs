use leptos::prelude::*;

use crate::components::Menu;

#[component]
pub fn Home() -> impl IntoView {
    view! {
        <>
            <Menu />
        </>
    }
}
