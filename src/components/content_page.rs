use common::RouteId;
use leptos::prelude::*;
use leptos_router::{hooks::use_navigate, NavigateOptions};
use leptos_use::use_window;

use crate::components::{Button, Card, Icon, UiPlacement, UiSize, UiVariant};

/// Generic Page component (client-driven).
#[component]
pub fn ContentPage(
    /// Page title
    #[prop(into, optional)]
    title: String,

    /// Page content
    children: Children,

    /// Animation
    #[prop(optional, into)]
    card_animation_direction: Signal<Option<UiPlacement>>,

    #[prop(optional, into)] no_back_button: bool,
) -> impl IntoView {
    let naviagte = use_navigate();

    let go_back = Callback::new(move |_| {
        if let Some(history) = use_window()
            .as_ref()
            .and_then(|w| w.history().ok())
            .filter(|h| h.length().is_ok_and(|l| l > 0))
        {
            let _ = history.back();
        } else {
            naviagte(RouteId::Home.as_ref(), NavigateOptions::default())
        }
    });

    view! {
        <div class="w-full h-full flex items-center justify-center">
            <Card class="max-h-screen" card_animation_direction>
                <div class="flex items-center justify-between gap-4 mb-6">
                    <Show when=move || !no_back_button>
                        <Button
                            size=UiSize::Md
                            variant=UiVariant::Outline
                            on:click=move |_| go_back.run(())
                        >
                            <Icon name="back" size=UiSize::Md />
                            <span class="inline-block ml-2 flex-grow text-center">Back</span>
                        </Button>
                    </Show>
                    <h1 class="block text-5xl text-center italic">{title}</h1>
                </div>
                {children()}
            </Card>
        </div>
    }
}
