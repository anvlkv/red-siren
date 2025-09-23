use leptos::prelude::*;

use crate::components::{Menu, Splash};

#[component]
pub fn Home() -> impl IntoView {
    view! {
        <>
            <Splash />
            <Menu />
        </>
    }
}
