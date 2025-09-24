use leptos::prelude::*;
use tauri_use::{use_invoke_with_args, UseTauriWithReturn};

use crate::components::{Icon, Tooltip};

#[derive(Clone, Debug)]
pub enum MenuItem {
    Navigate {
        path: String,
        icon: String,
        label: String,
    },
    Action {
        icon: String,
        label: String,
        action: String,
    },
}

impl Default for MenuItem {
    fn default() -> Self {
        MenuItem::Navigate {
            path: "/".to_string(),
            icon: "home".to_string(),
            label: "Home".to_string(),
        }
    }
}

#[component]
pub fn Menu(#[prop(into, optional)] compact: bool) -> impl IntoView {
    let items = RwSignal::new(vec![
        MenuItem::Navigate {
            path: "/play".to_string(),
            icon: "play".to_string(),
            label: "Play".to_string(),
        },
        MenuItem::Navigate {
            path: "/tune".to_string(),
            icon: "tune".to_string(),
            label: "Tune".to_string(),
        },
        MenuItem::Navigate {
            path: "/about".to_string(),
            icon: "info".to_string(),
            label: "About".to_string(),
        },
    ]);

    view! {
        <div class="inline-grid grid-cols-1 justify-center p-8 gap-4 w-max mx-auto bg-red dark:bg-black shadow-xl shadow-gray dark:shadow-cinnabar rounded-xl">
            <Show
                when=move || compact
                fallback=move || {
                    view! {
                        <h1 class="block text-5xl text-center">Red Siren</h1>
                        <nav class="contents text-red dark:text-black text-3xl">
                            {items()
                                .iter()
                                .map(|item| view! { <MenuItemView item=item.clone() /> })
                                .collect_view()}
                        </nav>
                    }
                }
            >
                {move || {
                    view! {
                        <nav class="contents text-red dark:text-black text-3xl">
                            <div class="flex flex-row items-center justify-center gap-4">
                                {items()
                                    .iter()
                                    .map(|item| view! { <MenuItemCompactView item=item.clone() /> })
                                    .collect_view()}
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
fn MenuItemCompactView(#[prop(into)] item: MenuItem) -> impl IntoView {
    let UseTauriWithReturn { trigger, .. } =
        use_invoke_with_args::<String, ()>(shared::commands::navigation::NAVIGATE);

    view! {
        <div class="rounded-full" role="menuitem">
            {match item {
                MenuItem::Navigate { path, icon, label } => {
                    let aria_label = label.clone();
                    view! {
                        <Tooltip text=label.clone() placement="top">
                            <button
                                type="button"
                                class="relative h-16 w-16 flex items-center justify-center rounded-full bg-black dark:bg-red text-red dark:text-black cursor-pointer transition-all duration-200 hover:scale-110 hover:shadow-md hover:shadow-gray dark:hover:shadow-cinnabar active:scale-105 active:shadow-sm active:shadow-gray dark:active:shadow-cinnabar"
                                role="button"
                                on:click=move |_| {
                                    trigger(Some(path.clone()));
                                }
                                aria-label=aria_label
                            >
                                <span class="text-4xl leading-none">
                                    <Icon name=icon stroke_width=12.0 />
                                </span>
                            </button>
                        </Tooltip>
                    }
                        .into_any()
                }
                MenuItem::Action { icon, label, action } => {
                    let aria_label = label.clone();
                    view! {
                        <Tooltip text=label.clone() placement="top">
                            <button
                                type="button"
                                class="relative h-16 w-16 flex items-center justify-center rounded-full bg-black dark:bg-red text-red dark:text-black cursor-pointer transition-all duration-200 hover:scale-110 hover:shadow-md hover:shadow-gray dark:hover:shadow-cinnabar active:scale-105 active:shadow-sm active:shadow-gray dark:active:shadow-cinnabar"
                                role="button"
                                on:click=move |_| {
                                    log::info!("Action triggered: {}", action);
                                }
                                aria-label=aria_label
                            >
                                <span class="text-4xl leading-none">
                                    <Icon name=icon stroke_width=12.0 />
                                </span>
                            </button>
                        </Tooltip>
                    }
                        .into_any()
                }
            }}
        </div>
    }
}

#[component]
fn MenuItemView(#[prop(into)] item: MenuItem) -> impl IntoView {
    let UseTauriWithReturn { trigger, .. } =
        use_invoke_with_args::<String, ()>(shared::commands::navigation::NAVIGATE);

    view! {
        <div class="rounded-lg" role="menuitem">
            {match item {
                MenuItem::Navigate { path, icon, label } => {
                    let aria_label = label.clone();
                    view! {
                        <button
                            type="button"
                            class="relative w-full flex p-4 pl-14 items-center justify-center bg-black dark:bg-red rounded-lg cursor-pointer transition-all duration-200 hover:scale-105 hover:shadow-md hover:shadow-gray dark:hover:shadow-cinnabar active:scale-100 active:shadow-sm active:shadow-gray dark:active:shadow-cinnabar"
                            role="button"
                            on:click=move |_| {
                                trigger(Some(path.clone()));
                            }
                            aria-label=aria_label
                        >
                            <span class="absolute left-4 text-4xl">
                                <Icon name=icon stroke_width=12.0 />
                            </span>
                            {label}
                        </button>
                    }
                        .into_any()
                }
                MenuItem::Action { icon, label, action } => {
                    let aria_label = label.clone();
                    view! {
                        <button
                            type="button"
                            class="relative w-full flex p-4 pl-14 items-center justify-center bg-black dark:bg-red rounded-lg cursor-pointer transition-all duration-200 hover:scale-105 hover:shadow-md hover:shadow-gray dark:hover:shadow-cinnabar active:scale-100 active:shadow-sm active:shadow-gray dark:active:shadow-cinnabar"
                            role="button"
                            on:click=move |_| {
                                log::info!("Action triggered: {}", action);
                            }
                            aria-label=aria_label
                        >
                            <span class="absolute left-4 text-4xl">
                                <Icon name=icon stroke_width=12.0 />
                            </span>
                            {label}
                        </button>
                    }
                        .into_any()
                }
            }}
        </div>
    }
}
