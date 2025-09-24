use leptos::prelude::*;

use crate::components::{Intro, Menu};

#[component]
pub fn Home() -> impl IntoView {
    view! {
        <>
            <Menu />
        </>
    }
}
