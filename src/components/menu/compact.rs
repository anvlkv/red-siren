use leptos::prelude::*;
use shared::commands::navigation::NavigateRequestPayload;
use tauri_use::{use_invoke_with_args, UseTauriWithReturn};

use crate::components::Card;

use super::item::{MenuItem, MenuItemView};

#[component]
pub fn CompactMenu(
    #[prop(into)] items: Signal<Vec<MenuItem>>,
    children: Children,
) -> impl IntoView {
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

    view! {
        <Card padding="Sm".to_string() class="rounded-b-none">
            <div class="flex gap-2">
                <h1 class="block text-3xl italic">"Red Siren"</h1>
                {children()}
                {move || {
                    items()
                        .iter()
                        .copied()
                        .map(|item| view! { <MenuItemView item trigger_navigate compact=true /> })
                        .collect_view()
                }}
            </div>
        </Card>
    }
}
