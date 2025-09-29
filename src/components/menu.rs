mod compact;
mod item;

use leptos::prelude::*;

use shared::commands::navigation::NavigateRequestPayload;
use tauri_use::{use_invoke_with_args, UseTauriWithReturn};

pub use compact::*;
pub use item::*;

#[component]
pub fn Menu() -> impl IntoView {
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
        <div class="inline-grid grid-cols-1 gap-4 w-full">
            <nav class="contents text-3xl">
                {DEFAULT_MENU_ITEMS
                    .iter()
                    .copied()
                    .map(|item| {
                        view! { <MenuItemView item trigger_navigate /> }
                    })
                    .collect_view()}
            </nav>
        </div>
    }
}
