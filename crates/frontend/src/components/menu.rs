mod compact;
mod item;

use common::RouteId;
use leptos::prelude::*;

use crate::util::boot_flags::boot_flags;

pub use compact::*;
pub use item::*;

#[component]
pub fn Menu() -> impl IntoView {
    let mut menu_items = Vec::from(DEFAULT_MENU_ITEMS);

    if boot_flags().devtools {
        menu_items.insert(
            0,
            MenuItem::Navigate {
                route: RouteId::Edit(common::EditorRouteId::Layout),
                icon: "probe",
                label: "Edit",
            },
        );
        menu_items.insert(
            1,
            MenuItem::Navigate {
                route: RouteId::TestNode,
                icon: "tune",
                label: "Test Node",
            },
        );
    }

    view! {
        <div class="inline-grid grid-cols-1 gap-4 w-full">
            <nav class="contents md:text-3xl text-xl">
                {menu_items
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
