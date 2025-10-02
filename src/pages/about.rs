use crate::components::{Button, ContentPage, Icon, UiSize};
use common::{commands::navigation::NavigateRequestPayload, RouteId};
use leptos::prelude::*;
use tauri_use::{use_invoke_with_args, UseTauriWithReturn};

#[component]
pub fn About() -> impl IntoView {
    let UseTauriWithReturn {
        trigger: navigate_trigger,
        error: navigate_error,
        ..
    } = use_invoke_with_args::<NavigateRequestPayload, ()>(common::commands::navigation::NAVIGATE);

    Effect::new(move |_| {
        if let Some(err) = navigate_error() {
            log::error!(
                "Error invoking {}: {}",
                common::commands::navigation::NAVIGATE,
                err
            );
        }
    });

    view! {
        <ContentPage route_id=RouteId::About title="About">
            <div class="flex flex-col items-center justify-center gap-6">
                <h2 class="text-2xl text-bold max-w-md lg:max-w-[42ch] italic">
                    "Red Siren is a noise chime"
                </h2>
                <p class="text-xl max-w-md lg:max-w-[42ch] ">
                    "It pulls the present into focus — a siren's call, loud and true. A sound that blooms, that bends, that sharpens to the edge you set. It is a siren that sings only when you do, a mirror of noise, a vessel of tone."
                </p>
                <p class="text-xl max-w-md lg:max-w-[42ch] ">
                    "It hums with what you give it — loud, brief, true. A thousand crystal bowls shattering into light, a frequency tuned to the shape of your breath. Strike it, and it strikes back. Call it, and it calls you forward."
                </p>
                <p class="text-xl max-w-md lg:max-w-[42ch] ">
                    "Red Siren is free and open source under the CC‑BY‑SA license — take the code, remix it, and share what you make. If it speaks to you, show some love:"
                    <a
                        class="font-bold ml-2 underline"
                        href="https://github.com/anvlkv/red-siren"
                        target="_blank"
                        aria-label="Star anvlkv/red-siren on GitHub"
                    >
                        "Star me on GitHub"
                    </a>
                </p>
                <Button
                    on:click=move |_| {
                        navigate_trigger(
                            Some(NavigateRequestPayload {
                                route: RouteId::Donate,
                            }),
                        )
                    }
                    attr:aria-label="Donations"
                    size=UiSize::Lg
                    class="w-full"
                >
                    <Icon name="donate" size=UiSize::Lg />
                    <span class="inline-block flex-grow text-center">"Donate"</span>
                </Button>
            </div>
        </ContentPage>
    }
}
