use common::{safe_area::SafeArea, RouteId};
use leptos::{html, prelude::*};
use leptos_router::components::A;
use leptos_use::{use_element_size, UseElementSizeReturn};

use tauri_use::use_invoke_with_args;

use crate::components::{Card, UiPlacement};

use super::item::{MenuItem, MenuItemView};

#[component]
pub fn CompactMenu(
    #[prop(into)] items: Signal<Vec<MenuItem>>,
    #[prop(into)] placement: Signal<UiPlacement>,
    children: Children,
) -> impl IntoView {
    // Materialize children once
    let child_view = children();

    let el = NodeRef::<html::Div>::new();

    let UseElementSizeReturn {
        width: menu_width,
        height: menu_height,
    } = use_element_size(el);

    let tauri_use::UseTauriWithReturn {
        trigger: trigger_inset_update,
        error: _inset_error,
        ..
    } = use_invoke_with_args::<common::safe_area::SafeArea, ()>(
        common::commands::setup::UI_SAFE_AREA_INSETS_APPLY,
    );

    // Update safe area insets when menu size or placement changes
    Effect::new(move |prev_insets: Option<SafeArea>| {
        let menu_height = menu_height() as f32;
        let menu_width = menu_width() as f32;
        let placement = placement();

        let new_insets = match placement {
            UiPlacement::Bottom => SafeArea {
                top: 0.0,
                right: 0.0,
                bottom: menu_height,
                left: 0.0,
            },
            UiPlacement::Top => SafeArea {
                top: menu_height,
                right: 0.0,
                bottom: 0.0,
                left: 0.0,
            },
            UiPlacement::Left => SafeArea {
                top: 0.0,
                right: 0.0,
                bottom: 0.0,
                left: menu_width,
            },
            UiPlacement::Right => SafeArea {
                top: 0.0,
                right: menu_width,
                bottom: 0.0,
                left: 0.0,
            },
        };

        // Only trigger if values have changed significantly (>1px threshold)
        let should_update = match prev_insets {
            None => true,
            Some(prev) => {
                (new_insets.top - prev.top).abs() > 1.0
                    || (new_insets.right - prev.right).abs() > 1.0
                    || (new_insets.bottom - prev.bottom).abs() > 1.0
                    || (new_insets.left - prev.left).abs() > 1.0
            }
        };

        if should_update {
            log::debug!("updating UI, safe area insets");
            trigger_inset_update(Some(new_insets));
        }

        new_insets
    });

    let is_vertical = Signal::derive(move || placement().is_vertical());

    // Derived CSS classes to prevent recalculation
    let edge_container_cls = Signal::derive(move || match placement() {
        UiPlacement::Bottom => "fixed inset-x-0 bottom-0 flex justify-center pointer-events-none",
        UiPlacement::Top => "fixed inset-x-0 top-0 flex justify-center pointer-events-none",
        UiPlacement::Left => "fixed min-w-5 inset-y-0 left-0 flex items-center pointer-events-none",
        UiPlacement::Right => {
            "fixed min-w-5 inset-y-0 right-0 flex items-center pointer-events-none"
        }
    });

    let card_variant = Signal::derive(move || {
        let place = placement();
        format!(
            "{} overflow-visible",
            match place {
                UiPlacement::Bottom => "rounded-b-none px-4",
                UiPlacement::Top => "rounded-t-none px-4",
                UiPlacement::Left => "rounded-l-none py-4",
                UiPlacement::Right => "rounded-r-none py-4",
            }
        )
    });

    let inner_flex_class = Signal::derive(move || {
        format!(
            "flex {} gap-4 pointer-events-auto",
            if is_vertical() {
                "flex-col items-center justify-center"
            } else {
                "flex-row items-center"
            }
        )
    });

    let title_style = Signal::derive(move || {
        if is_vertical() {
            "writing-mode: vertical-rl; text-orientation: mixed;"
        } else {
            ""
        }
    });

    view! {
        <div class=edge_container_cls node_ref=el>
            <Card padding="Sm".to_string() class=card_variant card_animation_direction=placement>
                <div class=inner_flex_class>
                    <A href=RouteId::Home.as_ref() attr:class="contents">
                        <h1
                            class="block md:text-3xl text-xl italic cursor-pointer hover:underline focus:underline"
                            style=title_style
                        >
                            "Red Siren"
                        </h1>
                    </A>
                    {child_view}
                    {move || {
                        items()
                            .iter()
                            .copied()
                            .map(|item| {
                                view! {
                                    <MenuItemView item compact=true menu_placement=placement />
                                }
                                    .into_any()
                            })
                            .collect_view()
                    }}
                </div>
            </Card>
        </div>
    }
}
