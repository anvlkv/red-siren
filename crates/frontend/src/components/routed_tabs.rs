use common::RouteId;
use leptos::prelude::*;
use leptos_router::{
    components::{Outlet, A},
    hooks::use_navigate,
    NavigateOptions,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct RoutedTab {
    pub label: &'static str,
    pub route: RouteId,
}

#[component]
pub fn RoutedTabs(
    #[prop(into)] tabs: Signal<Vec<RoutedTab>>,
    #[prop(optional, into)] class: Signal<String>,
) -> impl IntoView {
    let root_class = move || {
        let extra = class();
        if extra.is_empty() {
            "w-full h-full min-h-0 flex flex-col overflow-hidden".to_string()
        } else {
            extra
        }
    };

    view! {
        <section class=root_class>
            <div class="w-full shrink-0">
                <nav
                    class="flex w-full items-end gap-2 overflow-visible px-1 pt-1"
                    aria-label="Editor sections"
                >
                    <For
                        each=move || tabs()
                        key=|tab| tab.route.path().to_string()
                        children=move |tab| {
                            let path = tab.route.path().to_string();
                            let navigate = use_navigate();

                            view! {
                                <A
                                    href=path.clone()
                                    exact=true
                                    on:click=move |ev| {
                                        ev.prevent_default();
                                        navigate(
                                            path.as_str(),
                                            NavigateOptions {
                                                replace: true,
                                                ..Default::default()
                                            },
                                        )
                                    }
                                    attr:class="group inline-flex min-h-11 min-w-[12rem] max-w-full shrink-0 items-center justify-center rounded-t-xl border-x-2 border-t-2 border-black/25 bg-black/5 px-4 py-3 text-center text-base italic transition-colors transition-shadow duration-200 hover:bg-black/10 hover:shadow-md hover:shadow-gray focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-offset-2 dark:border-red/25 dark:bg-red/5 dark:hover:bg-red/10 dark:hover:shadow-cinnabar aria-[current=page]:-mb-px aria-[current=page]:border-black aria-[current=page]:bg-black aria-[current=page]:text-red aria-[current=page]:shadow-md aria-[current=page]:shadow-gray dark:aria-[current=page]:border-red dark:aria-[current=page]:bg-red dark:aria-[current=page]:text-black dark:aria-[current=page]:shadow-cinnabar"
                                >
                                    <span class="truncate">{tab.label}</span>
                                </A>
                            }
                        }
                    />
                </nav>
            </div>

            <div class="min-h-0 flex-1 overflow-hidden rounded-b-xl rounded-tl-none rounded-tr-xl border-2 border-black/20 bg-black/5 dark:border-red/20 dark:bg-red/5">
                <Outlet />
            </div>
        </section>
    }
}
