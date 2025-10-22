use common::RouteId;
use leptos::prelude::*;
use leptos_router::{hooks::use_navigate, NavigateOptions};

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

    /// Extra classes for the Card
    #[prop(optional, into)]
    card_class: Signal<String>,

    /// Use special first-appear animation
    #[prop(optional)]
    first_appear: bool,

    #[prop(optional, into)] no_back_button: bool,
) -> impl IntoView {
    let navigate = use_navigate();

    // Merge base card class with user-supplied class
    let merged_card_class = Signal::derive(move || {
        let extra = card_class();
        if extra.is_empty() {
            "max-h-screen".to_string()
        } else {
            format!("{} {}", "max-h-screen", extra)
        }
    });
    let nav_stack = use_context::<StoredValue<Vec<String>>>();

    let go_back = Callback::new(move |_| {
        if let Some(stack) = nav_stack.as_ref() {
            // Pop current and navigate to the previous in-app path if present
            let mut prev: Option<String> = None;
            stack.update_value(|s| {
                if s.len() > 1 {
                    s.pop();
                    prev = s.last().cloned();
                }
            });
            if let Some(path) = prev {
                navigate(
                    &path,
                    NavigateOptions {
                        replace: true,
                        ..Default::default()
                    },
                );
                return;
            }
        }
        // Fallback to Home when no previous in-app entry exists
        navigate(
            RouteId::Home.as_ref(),
            NavigateOptions {
                replace: true,
                ..Default::default()
            },
        );
    });

    view! {
        <div class="w-full h-full flex items-center justify-center">
            <Card class=merged_card_class card_animation_direction first_appear=first_appear>
                <div class="flex items-center justify-between flex-wrap gap-4 mb-6 w-full">
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
                    <h1 class="block flex-grow md:text-5xl text-2xl text-center italic">{title}</h1>
                </div>
                {children()}
            </Card>
        </div>
    }
}
