use leptos::{ev, html, prelude::*};
use leptos_use::{use_document, use_event_listener};
use wasm_bindgen::JsCast;

use crate::components::UiPlacement;

fn menu_items(panel: &web_sys::Element) -> Vec<web_sys::HtmlElement> {
    let selector = "button:not([disabled]), [role='menuitem'], [role='menuitemradio'], [role='menuitemcheckbox'], a[href]";
    let Ok(nodes) = panel.query_selector_all(selector) else {
        return Vec::new();
    };

    let mut items = Vec::with_capacity(nodes.length() as usize);
    for index in 0..nodes.length() {
        let Some(node) = nodes.get(index) else {
            continue;
        };
        let Ok(item) = node.dyn_into::<web_sys::HtmlElement>() else {
            continue;
        };
        items.push(item);
    }
    items
}

/// Placement-aware dropdown panel.
///
/// Renders its own trigger and manages open/close internally.
/// The dropdown closes on outside pointer down, item activation, and Escape.
#[component]
pub fn Dropdown(
    trigger: AnyView,
    children: ChildrenFn,
    #[prop(optional, into)] placement: Signal<Option<UiPlacement>>,
    #[prop(optional, into)] class: Signal<String>,
    #[prop(optional, into)] trigger_class: Signal<String>,
) -> impl IntoView {
    let document = use_document();
    let (open, set_open) = signal(false);
    let trigger_ref = NodeRef::<html::Button>::new();
    let panel_ref = NodeRef::<html::Div>::new();

    let root_class = Signal::derive(|| "relative inline-flex".to_string());

    let panel_position = Signal::derive(move || match placement().unwrap_or(UiPlacement::Top) {
        UiPlacement::Bottom => "left-1/2 top-full mt-2 -translate-x-1/2",
        UiPlacement::Top => "left-1/2 bottom-full mb-2 -translate-x-1/2",
        UiPlacement::Left => "right-full top-1/2 -translate-y-1/2 mr-2",
        UiPlacement::Right => "left-full top-1/2 -translate-y-1/2 ml-2",
    });

    let panel_class = Signal::derive(move || {
        format!(
            "absolute z-[120] min-w-28 rounded-lg border-2 border-black dark:border-red bg-red/95 text-black dark:bg-black/95 dark:text-red shadow-xl p-1 backdrop-blur-sm {} {}",
            panel_position(),
            class()
        )
    });

    let trigger_button_class = Signal::derive(move || {
        format!(
            "inline-flex h-full w-full items-center justify-center focus:outline-none {}",
            trigger_class()
        )
    });

    let _close_on_pointer_down = use_event_listener(use_document(), ev::pointerdown, {
        let panel_ref = panel_ref;
        let trigger_ref = trigger_ref;
        move |event: web_sys::PointerEvent| {
            if !open.get_untracked() {
                return;
            }

            let Some(panel) = panel_ref.get_untracked() else {
                return;
            };
            let trigger = trigger_ref.get_untracked();

            let Some(target) = event.target() else {
                set_open.set(false);
                return;
            };

            let Ok(target_node) = target.dyn_into::<web_sys::Node>() else {
                set_open.set(false);
                return;
            };

            let clicked_panel = panel.contains(Some(&target_node));
            let clicked_trigger = trigger
                .as_ref()
                .is_some_and(|trigger| trigger.contains(Some(&target_node)));

            if !clicked_panel && !clicked_trigger {
                set_open.set(false);
            }
        }
    });

    let _close_on_escape = use_event_listener(
        document.clone(),
        ev::keydown,
        move |event: web_sys::KeyboardEvent| {
            if open.get_untracked() && event.key() == "Escape" {
                event.prevent_default();
                set_open.set(false);
            }
        },
    );

    Effect::new(move |was_open| {
        let is_open = open();
        if !is_open || was_open == Some(true) {
            return is_open;
        }

        let Some(panel) = panel_ref.get() else {
            return is_open;
        };

        if let Ok(Some(selected)) = panel.query_selector(
            "[role='menuitemradio'][aria-checked='true'], [role='menuitemcheckbox'][aria-checked='true']",
        ) {
            if let Ok(selected_item) = selected.dyn_into::<web_sys::HtmlElement>() {
                let _ = selected_item.focus();
                return is_open;
            }
        }

        let items = menu_items(panel.as_ref());
        if let Some(first) = items.first() {
            let _ = first.focus();
        } else if let Ok(panel_element) = panel.dyn_into::<web_sys::HtmlElement>() {
            let _ = panel_element.focus();
        }

        is_open
    });

    Effect::new(move |was_open| {
        let is_open = open();
        if is_open || was_open != Some(true) {
            return is_open;
        }

        if let Some(trigger) = trigger_ref.get() {
            let _ = trigger.focus();
        }

        is_open
    });

    view! {
        <div class=root_class>
            <button
                type="button"
                node_ref=trigger_ref
                class=trigger_button_class
                aria-expanded=open
                aria-haspopup="menu"
                on:click=move |_| {
                    set_open.update(|is_open| *is_open = !*is_open);
                }
                on:keydown=move |event: web_sys::KeyboardEvent| {
                    match event.key().as_str() {
                        "ArrowDown" | "ArrowUp" | "Enter" | " " | "Spacebar" => {
                            event.prevent_default();
                            set_open.set(true);
                        }
                        _ => {}
                    }
                }
            >
                {trigger}
            </button>

            <Show when=move || open()>
                <div
                    node_ref=panel_ref
                    class=panel_class
                    role="menu"
                    tabindex="-1"
                    on:click=move |event: web_sys::MouseEvent| {
                        let Some(target) = event.target() else {
                            return;
                        };
                        let Ok(element) = target.dyn_into::<web_sys::Element>() else {
                            return;
                        };
                        if element
                            .closest(
                                "button, [role='menuitem'], [role='menuitemradio'], [role='menuitemcheckbox'], a[href]",
                            )
                            .ok()
                            .flatten()
                            .is_some()
                        {
                            set_open.set(false);
                        }
                    }
                    on:keydown={
                        let document = document.clone();
                        move |event: web_sys::KeyboardEvent| {
                            let Some(panel) = panel_ref.get_untracked() else {
                                return;
                            };
                            let items = menu_items(panel.as_ref());
                            if items.is_empty() {
                                if event.key() == "Escape" {
                                    event.prevent_default();
                                    set_open.set(false);
                                }
                                return;
                            }
                            let focused_index = document
                                .active_element()
                                .and_then(|active| {
                                    items
                                        .iter()
                                        .position(|item| active.is_same_node(Some(item.as_ref())))
                                });
                            match event.key().as_str() {
                                "ArrowDown" => {
                                    event.prevent_default();
                                    let next = focused_index
                                        .map(|index| (index + 1) % items.len())
                                        .unwrap_or(0);
                                    let _ = items[next].focus();
                                }
                                "ArrowUp" => {
                                    event.prevent_default();
                                    let next = focused_index
                                        .map(|index| {
                                            if index == 0 { items.len() - 1 } else { index - 1 }
                                        })
                                        .unwrap_or(items.len() - 1);
                                    let _ = items[next].focus();
                                }
                                "Home" => {
                                    event.prevent_default();
                                    let _ = items[0].focus();
                                }
                                "End" => {
                                    event.prevent_default();
                                    let _ = items[items.len() - 1].focus();
                                }
                                "Enter" | " " | "Spacebar" => {
                                    if let Some(index) = focused_index {
                                        event.prevent_default();
                                        items[index].click();
                                    }
                                }
                                "Escape" => {
                                    event.prevent_default();
                                    set_open.set(false);
                                }
                                _ => {}
                            }
                        }
                    }
                >
                    {children()}
                </div>
            </Show>
        </div>
    }
}
