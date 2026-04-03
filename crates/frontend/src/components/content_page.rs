use leptos::prelude::*;
use tauri_use::{use_command, UseTauriWithReturn};

use crate::{
    components::{Button, Card, Icon, UiPlacement, UiSize, UiVariant},
    util::secondary_window::is_secondary_window,
};

/// Generic Page component (client-driven).
#[component]
pub fn ContentPage(
    /// Page title
    #[prop(into, optional)]
    title: Signal<String>,

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
    let is_secondary_window = is_secondary_window();
    let UseTauriWithReturn {
        trigger: trigger_go_back,
        error: error_go_back,
        ..
    } = use_command::<()>(common::commands::setup::GO_BACK);

    // Merge base card class with user-supplied class
    let merged_card_class = Signal::derive(move || {
        let extra = card_class();
        if extra.is_empty() {
            "max-h-screen".to_string()
        } else {
            format!("{} {}", "max-h-screen", extra)
        }
    });
    Effect::new(move |_| {
        if let Some(err) = error_go_back() {
            log::error!("Error invoking {}: {err}", common::commands::setup::GO_BACK);
        }
    });

    let go_back = Callback::new(move |_| {
        trigger_go_back(Some(()));
    });

    let show_back_button = Signal::derive(move || !no_back_button && !is_secondary_window());

    view! {
        <div class=move || {
            format!(
                "{} flex items-center justify-center",
                if is_secondary_window() { "w-screen h-screen" } else { "w-full h-full" },
            )
        }>
            <Card class=merged_card_class card_animation_direction first_appear=first_appear>
                <div class="flex items-center justify-between flex-wrap gap-4 mb-6 w-full">
                    <Show when=move || show_back_button()>
                        <Button
                            size=UiSize::Md
                            variant=UiVariant::Outline
                            on:click=move |_| go_back.run(())
                        >
                            <Icon name="back" size=UiSize::Md />
                            <span class="inline-block ml-2 flex-grow text-center">Back</span>
                        </Button>
                    </Show>
                    <h1 class=move || {
                        format!(
                            "block flex-grow md:text-5xl text-2xl italic {}",
                            if show_back_button() { "text-right" } else { "text-center" },
                        )
                    }>{title}</h1>
                </div>
                {children()}
            </Card>
        </div>
    }
}
