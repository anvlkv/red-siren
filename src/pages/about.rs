use crate::components::{Button, Icon, Page};
use leptos::prelude::*;
use shared::{commands::navigation::NavigateRequestPayload, RouteId};
use tauri_use::{use_invoke_with_args, UseTauriWithReturn};

#[component]
pub fn About() -> impl IntoView {
    let UseTauriWithReturn {
        trigger: navigate_trigger,
        error: navigate_error,
        ..
    } = use_invoke_with_args::<NavigateRequestPayload, ()>(shared::commands::navigation::NAVIGATE);

    Effect::new(move |_| {
        if let Some(err) = navigate_error() {
            log::error!(
                "Error invoking {}: {}",
                shared::commands::navigation::NAVIGATE,
                err
            );
        }
    });

    view! {
        <Page route_id=RouteId::About title="About" route_back=RouteId::Home>
            <div class="flex flex-col items-center justify-center gap-6">
                <h2 class="text-2xl text-bold max-w-[42ch] italic">"Red Siren is a noise chime"</h2>
                <p class="text-xl max-w-[42ch] ">
                    "It pulls the present into focus — a siren's call, loud and true. A sound that blooms, that bends, that sharpens to the edge you set. It is a siren that sings only when you do, a mirror of noise, a vessel of tone."
                </p>
                <p class="text-xl max-w-[42ch] ">
                    "It hums with what you give it — loud, brief, true. A thousand crystal bowls shattering into light, a frequency tuned to the shape of your breath. Strike it, and it strikes back. Call it, and it calls you forward."
                </p>
                <Button
                    on:click=move |_| {
                        navigate_trigger(
                            Some(NavigateRequestPayload {
                                route: RouteId::Donate,
                            }),
                        )
                    }
                    full_width=true
                    class="relative pl-14"
                    attr:aria-label="Donations"
                >
                    <span class="absolute left-4 text-4xl">
                        <Icon name="donate" stroke_width=12.0 />
                    </span>
                    "Donate"
                </Button>
            </div>
        </Page>
    }
}
