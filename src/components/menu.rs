use leptos::prelude::*;
use leptos_router::components::A;

use crate::components::{Icon, Tooltip};

#[component]
pub fn Menu(#[prop(into, optional)] compact: bool) -> impl IntoView {
    view! {
        <div class="inline-grid grid-cols-1 justify-center p-8 gap-4 w-max mx-auto bg-red dark:bg-black shadow-xl shadow-gray dark:shadow-cinnabar rounded-xl">
            <Show
                when=move || compact
                fallback=|| {
                    view! {
                        <h1 class="block text-5xl text-center">Red Siren</h1>
                        <nav class="contents text-red dark:text-black text-3xl">
                            <MenuItem path="/play" icon="play" label="Play" />
                            <MenuItem path="/tune" icon="tune" label="Tune" />
                            <MenuItem path="/about" icon="info" label="About" />
                        </nav>
                    }
                }
            >
                {move || {
                    view! {
                        <nav class="contents text-red dark:text-black text-3xl">
                            <div class="flex flex-row items-center justify-center gap-4">
                                <MenuItemCompact path="/play" icon="play" label="Play" />
                                <MenuItemCompact path="/tune" icon="tune" label="Tune" />
                                <MenuItemCompact path="/about" icon="info" label="About" />
                            </div>
                        </nav>
                        <h1 class="block text-2xl text-center">Red Siren</h1>
                    }
                }}
            </Show>
        </div>
    }
}

#[component]
fn MenuItemCompact(
    #[prop(into)] path: String,
    #[prop(into)] label: String,
    #[prop(into)] icon: String,
) -> impl IntoView {
    view! {
        <div class="rounded-full has-[[aria-current=page]]:opacity-60 hover:outline hover:outline-2 hover:outline-cinnabar/40 focus-within:outline focus-within:outline-2 focus-within:outline-cinnabar/40">
            <Tooltip text=label placement="top">
                <A
                    attr:class="relative h-16 w-16 flex items-center justify-center rounded-full bg-black dark:bg-red text-red dark:text-black"
                    href=path
                >
                    <span class="text-4xl leading-none">
                        <Icon name=icon stroke_width=12.0 />
                    </span>
                </A>
            </Tooltip>
        </div>
    }
}

#[component]
fn MenuItem(
    #[prop(into)] path: String,
    #[prop(into)] label: String,
    #[prop(into)] icon: String,
) -> impl IntoView {
    view! {
        <div class="rounded-lg has-[[aria-current=page]]:opacity-60 hover:outline hover:outline-2 hover:outline-cinnabar/40 focus-within:outline focus-within:outline-2 focus-within:outline-cinnabar/40">
            <A
                attr:class="relative w-full flex p-4 pl-14 items-center justify-center bg-black dark:bg-red rounded-lg"
                href=path
            >
                <span class="absolute left-4 text-4xl">
                    <Icon name=icon stroke_width=12.0 />
                </span>
                {label}
            </A>
        </div>
    }
}
