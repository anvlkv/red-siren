use leptos::prelude::*;
use shared::{commands::navigation::NavigateRequestPayload, RouteId};

use crate::components::{Button, Icon, Tooltip, UiSize};

#[derive(Clone, Debug, Copy)]
pub enum MenuItem {
    Navigate {
        route: RouteId,
        icon: &'static str,
        label: &'static str,
    },
    Action {
        icon: &'static str,
        label: &'static str,
        action: Callback<()>,
    },
}

impl Default for MenuItem {
    fn default() -> Self {
        MenuItem::Navigate {
            route: RouteId::Home,
            icon: "home",
            label: "Home",
        }
    }
}

pub static DEFAULT_MENU_ITEMS: [MenuItem; 3] = [
    MenuItem::Navigate {
        route: RouteId::Play,
        icon: "play",
        label: "Play",
    },
    MenuItem::Navigate {
        route: RouteId::Tune,
        icon: "tune",
        label: "Tune",
    },
    MenuItem::Navigate {
        route: RouteId::About,
        icon: "info",
        label: "About",
    },
];

#[component]
pub fn MenuItemView(
    #[prop(into)] item: MenuItem,
    trigger_navigate: WriteSignal<Option<NavigateRequestPayload>>,
    #[prop(into, optional)] compact: bool,
) -> impl IntoView {
    let size = if compact { UiSize::Sm } else { UiSize::Lg };
    view! {
        <div class=move || if compact { "rounded-full" } else { "rounded-lg" } role="menuitem">
            {match item {
                MenuItem::Navigate { route, icon, label } => {
                    let aria_label = label;
                    if compact {
                        view! {
                            <Tooltip text=label placement="top">
                                <Button
                                    on:click=move |_| {
                                        log::debug!("Trigger navigate to: {route}");
                                        trigger_navigate(Some(NavigateRequestPayload { route }));
                                    }
                                    round=true
                                    square=true
                                    size
                                    attr:aria-label=aria_label
                                >
                                    <span class="text-4xl leading-none">
                                        <Icon name=icon size />
                                    </span>
                                </Button>
                            </Tooltip>
                        }
                            .into_any()
                    } else {
                        view! {
                            <Button
                                on:click=move |_| {
                                    log::debug!("Trigger navigate to: {route}");
                                    trigger_navigate(Some(NavigateRequestPayload { route }));
                                }
                                class="relative pl-14 w-full"
                                attr:aria-label=aria_label
                                size
                            >
                                <span class="absolute left-4 text-4xl">
                                    <Icon name=icon size />
                                </span>
                                {label}
                            </Button>
                        }
                            .into_any()
                    }
                }
                MenuItem::Action { icon, label, action } => {
                    let aria_label = label;
                    if compact {
                        view! {
                            <Tooltip text=label placement="top">
                                <Button
                                    on:click=move |_| {
                                        log::info!("Action triggered: {label}");
                                        action.run(())
                                    }
                                    round=true
                                    square=true
                                    size
                                    attr:aria-label=aria_label
                                >
                                    <span class="text-4xl leading-none">
                                        <Icon name=icon size />
                                    </span>
                                </Button>
                            </Tooltip>
                        }
                            .into_any()
                    } else {
                        view! {
                            <Button
                                on:click=move |_| {
                                    log::info!("Action triggered: {label}");
                                    action.run(())
                                }
                                class="relative pl-14 w-full"
                                attr:aria-label=Some(aria_label)
                            >
                                <span class="absolute left-4 text-4xl">
                                    <Icon name=icon />
                                </span>
                                {label}
                            </Button>
                        }
                            .into_any()
                    }
                }
            }}
        </div>
    }
}
