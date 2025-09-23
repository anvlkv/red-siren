use leptos::prelude::*;
use leptos_router::components::A;

#[component]
pub fn Menu() -> impl IntoView {
    view! {
        <div class="flex flex-col">
            <h1>Red Siren</h1>
            <nav class="contents">
                <A href="/play">Play</A>
                <A href="/tune">Tune</A>
                <A href="/about">About</A>
            </nav>
        </div>
    }
}
