mod compact;
mod item;

use leptos::prelude::*;

pub use compact::*;
pub use item::*;

#[component]
pub fn Menu() -> impl IntoView {
    view! {
        <div class="inline-grid grid-cols-1 gap-4 w-full">
            <nav class="contents text-3xl">
                {DEFAULT_MENU_ITEMS
                    .iter()
                    .copied()
                    .map(|item| {
                        view! { <MenuItemView item /> }
                    })
                    .collect_view()}
            </nav>
        </div>
    }
}
