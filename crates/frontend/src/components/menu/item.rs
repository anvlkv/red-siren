use common::RouteId;
use leptos::prelude::*;

use crate::components::{Button, Icon, Tooltip, UiPlacement, UiSize};

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

impl MenuItem {
    pub fn icon(&self) -> &'static str {
        match self {
            MenuItem::Navigate { icon, .. } => icon,
            MenuItem::Action { icon, .. } => icon,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            MenuItem::Navigate { label, .. } => label,
            MenuItem::Action { label, .. } => label,
        }
    }
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

pub static DEFAULT_MENU_ITEMS: [MenuItem; 4] = [
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
    MenuItem::Navigate {
        route: RouteId::Donate,
        icon: "donate",
        label: "Help",
    },
];

#[component]
pub fn MenuItemView(
    #[prop(into)] item: MenuItem,

    #[prop(into, optional)] compact: bool,
    #[prop(optional, into)] menu_placement: Signal<Option<UiPlacement>>,
) -> impl IntoView {
    let size = if compact { UiSize::Sm } else { UiSize::Lg };
    let label = item.label();
    let icon = item.icon();
    // Derive tooltip placement: opposite of menu edge when provided, else default Top.
    let tooltip_placement = Signal::derive(move || {
        menu_placement()
            .map(|p| p.opposite())
            .or(Some(UiPlacement::Top))
    });
    let on_click = move |_| match item {
        MenuItem::Action { action, .. } => {
            log::info!("Action triggered: {label}");
            action.run(())
        }
        MenuItem::Navigate { .. } => {
            // Navigation handled by <A> link; no-op for button click
        }
    };

    view! {
        <div class=move || if compact { "rounded-full" } else { "rounded-lg" } role="menuitem">
            {match item {
                MenuItem::Navigate { route, .. } => {
                    let href: &'static str = route.into();
                    if compact {

                        view! {
                            <Tooltip text=label placement=tooltip_placement>
                                <Button href=route.as_ref() square=true size>
                                    <Icon name=icon size=size />
                                </Button>
                            </Tooltip>
                        }
                            .into_any()
                    } else {
                        view! {
                            <Button href=route.as_ref() class="w-full justify-between" size>
                                <Icon name=icon size=size />
                                <span class="inline-block flex-grow text-center">{label}</span>
                            </Button>
                        }
                            .into_any()
                    }
                }
                MenuItem::Action { .. } => {
                    if compact {
                        view! {
                            <Tooltip text=label placement=tooltip_placement>
                                <Button on:click=on_click square=true size>
                                    <Icon name=icon size=size />
                                </Button>
                            </Tooltip>
                        }
                            .into_any()
                    } else {
                        view! {
                            <Button on:click=on_click class="w-full justify-between" size>
                                <Icon name=icon size=size />
                                <span class="inline-block flex-grow text-center">{label}</span>
                            </Button>
                        }
                            .into_any()
                    }
                }
            }}
        </div>
    }
}
