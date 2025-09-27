use leptos::prelude::*;

use shared::{commands::navigation::NavigateRequestPayload, navigation::RouteId};
use tauri_use::{use_invoke_with_args, UseTauriWithReturn};

use crate::components::{Button, Icon, Tooltip};

#[derive(Clone, Debug)]
pub enum MenuItem {
    Navigate {
        route: RouteId,
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
            route: RouteId::Home,
            icon: "home".to_string(),
            label: "Home".to_string(),
        }
    }
}

#[component]
pub fn Menu(#[prop(into, optional)] compact: bool) -> impl IntoView {
    let UseTauriWithReturn {
        trigger: trigger_navigate,
        error,
        ..
    } = use_invoke_with_args::<NavigateRequestPayload, ()>(shared::commands::navigation::NAVIGATE);

    Effect::new(move |_| {
        if let Some(err) = error() {
            log::error!(
                "Error invoking {}: {}",
                shared::commands::navigation::NAVIGATE,
                err
            );
        }
    });

    let items = RwSignal::new(vec![
        MenuItem::Navigate {
            route: RouteId::Home,
            icon: "play".to_string(),
            label: "Play".to_string(),
        },
        MenuItem::Navigate {
            route: RouteId::Tune,
            icon: "tune".to_string(),
            label: "Tune".to_string(),
        },
        MenuItem::Navigate {
            route: RouteId::About,
            icon: "info".to_string(),
            label: "About".to_string(),
        },
    ]);

    view! {
        <div class="inline-grid grid-cols-1 gap-4 w-full">
            <Show
                when=move || compact
                fallback=move || {
                    view! {
                        <nav class="contents text-3xl">
                            {items()
                                .iter()
                                .map(|item| {
                                    view! { <MenuItemView item=item.clone() trigger_navigate /> }
                                })
                                .collect_view()}
                        </nav>
                    }
                }
            >
                {move || {
                    view! {
                        <nav class="contents text-3xl">
                            <div class="flex flex-row items-center justify-center gap-4">
                                {items()
                                    .iter()
                                    .map(|item| {
                                        view! {
                                            <MenuItemCompactView item=item.clone() trigger_navigate />
                                        }
                                    })
                                    .collect_view()}
                            </div>
                        </nav>
                        <h1 class="block text-2xl text-center italic">Red Siren</h1>
                    }
                }}
            </Show>
        </div>
    }
}

#[component]
fn MenuItemView(
    #[prop(into)] item: MenuItem,
    trigger_navigate: WriteSignal<Option<NavigateRequestPayload>>,
) -> impl IntoView {
    view! {
        <div class="rounded-lg" role="menuitem">
            {match item {
                MenuItem::Navigate { route, icon, label } => {
                    let aria_label = label.clone();
                    view! {
                        <Button
                            on:click=move |_| {
                                log::debug!("Trigger navigate to: {route}");
                                trigger_navigate(Some(NavigateRequestPayload { route }));
                            }
                            full_width=true
                            class="relative pl-14"
                            attr:aria-label=aria_label.clone()
                        >
                            <span class="absolute left-4 text-4xl">
                                <Icon name=icon stroke_width=12.0 />
                            </span>
                            {label}
                        </Button>
                    }
                        .into_any()
                }
                MenuItem::Action { icon, label, action } => {
                    let aria_label = label.clone();
                    view! {
                        <Button
                            on:click=move |_| {
                                log::info!("Action triggered: {}", action);
                            }
                            full_width=true
                            class="relative pl-14"
                            attr:aria-label=Some(aria_label.clone())
                        >
                            <span class="absolute left-4 text-4xl">
                                <Icon name=icon stroke_width=12.0 />
                            </span>
                            {label}
                        </Button>
                    }
                        .into_any()
                }
            }}
        </div>
    }
}

#[component]
fn MenuItemCompactView(
    #[prop(into)] item: MenuItem,
    trigger_navigate: WriteSignal<Option<NavigateRequestPayload>>,
) -> impl IntoView {
    view! {
        <div class="rounded-full" role="menuitem">
            {match item {
                MenuItem::Navigate { route, icon, label } => {
                    let aria_label = label.clone();
                    view! {
                        <Tooltip text=label.clone() placement="top">
                            <Button
                                on:click=move |_| {
                                    log::debug!("Trigger navigate to: {route}");
                                    trigger_navigate(Some(NavigateRequestPayload { route }));
                                }
                                round=true
                                square=true
                                size=crate::components::ButtonSize::Lg
                                attr:aria-label=aria_label.clone()
                            >
                                <span class="text-4xl leading-none">
                                    <Icon name=icon stroke_width=12.0 />
                                </span>
                            </Button>
                        </Tooltip>
                    }
                        .into_any()
                }
                MenuItem::Action { icon, label, action } => {
                    let aria_label = label.clone();
                    view! {
                        <Tooltip text=label.clone() placement="top">
                            <Button
                                on:click=move |_| {
                                    log::info!("Action triggered: {}", action);
                                }
                                round=true
                                square=true
                                size=crate::components::ButtonSize::Lg
                                attr:aria-label=aria_label.clone()
                            >
                                <span class="text-4xl leading-none">
                                    <Icon name=icon stroke_width=12.0 />
                                </span>
                            </Button>
                        </Tooltip>
                    }
                        .into_any()
                }
            }}
        </div>
    }
}
