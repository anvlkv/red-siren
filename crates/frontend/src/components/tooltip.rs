use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicUsize, Ordering},
        OnceLock,
    },
};

use crate::components::UiPlacement;
use common::Rect;
use leptos::prelude::*;
use leptos_use::{
    use_element_bounding, use_mouse_in_element, UseElementBoundingReturn, UseMouseInElementReturn,
};

#[component]
fn Tooltip(
    text: Signal<String>,
    placement: Signal<UiPlacement>,
    rect: ReadSignal<Rect>,
) -> impl IntoView {
    let tooltip_classes = move || {
        let places_class = match placement() {
            UiPlacement::Bottom => "left-1/2 top-full mt-3 -translate-x-1/2",
            UiPlacement::Left => "right-full top-1/2 -translate-y-1/2 mr-3",
            UiPlacement::Right => "left-full top-1/2 -translate-y-1/2 ml-3",
            UiPlacement::Top => "left-1/2 bottom-full mb-3 -translate-x-1/2 -translate-y-full",
        };

        format!(
            "pointer-events-none absolute {} rounded-lg bg-black text-red dark:bg-red dark:text-black px-2 py-1 md:text-base text-sm whitespace-nowrap h-min w-min shadow-lg",
            places_class,
        )
    };
    let arrow_classes = move || {
        let places_class = match placement() {
            UiPlacement::Bottom => "left-1/2 -translate-x-1/2 -top-2",
            UiPlacement::Left => "top-1/2 -translate-y-1/2 -right-2",
            UiPlacement::Right => "top-1/2 -translate-y-1/2 -left-2",
            UiPlacement::Top => "left-1/2 -translate-x-1/2 -bottom-2",
        };
        format!(
            "absolute w-3 h-3 rotate-45 border-[8px] border-black dark:border-red {}",
            places_class,
        )
    };

    let tooltip_top = Signal::derive(move || {
        let (top_left, size) = rect();
        match placement() {
            UiPlacement::Top => format!("{}px", top_left.y),
            UiPlacement::Bottom => format!("{}px", top_left.y + size.y),
            UiPlacement::Left | UiPlacement::Right => format!("{}px", top_left.y + size.y / 2.0),
        }
    });
    let tooltip_left = Signal::derive(move || {
        let (top_left, size) = rect();
        match placement() {
            UiPlacement::Top | UiPlacement::Bottom => format!("{}px", top_left.x + size.x / 2.0),
            UiPlacement::Left => format!("{}px", top_left.x),
            UiPlacement::Right => format!("{}px", top_left.x + size.x),
        }
    });

    view! {
        <div role="tooltip" class=tooltip_classes style:top=tooltip_top style:left=tooltip_left>
            {text}
            <span aria-hidden="true" class=arrow_classes></span>
        </div>
    }
}

static TOOLTIP_ID: OnceLock<AtomicUsize> = OnceLock::new();

#[derive(Debug, Clone, PartialEq)]
struct TooltipEntry {
    placement: Signal<UiPlacement>,
    content: Signal<String>,
    ui_rect: RwSignal<Rect>,
}

#[derive(Clone)]
struct TooltipsContext(RwSignal<BTreeMap<usize, TooltipEntry>>);

pub fn provide_tooltips_context() {
    provide_context(TooltipsContext(RwSignal::new(BTreeMap::default())))
}

pub fn with_tooltip<El, M>(
    target: El,
    content: Signal<String>,
    placement: Signal<Option<UiPlacement>>,
) where
    El: leptos_use::core::IntoElementMaybeSignal<web_sys::Element, M> + Clone,
{
    let TooltipsContext(visible_tooltips) = expect_context();
    let UseMouseInElementReturn { is_outside, .. } = use_mouse_in_element(target.clone());
    let UseElementBoundingReturn {
        x,
        y,
        width,
        height,
        ..
    } = use_element_bounding(target);
    let id = TOOLTIP_ID
        .get_or_init(|| AtomicUsize::new(0))
        .fetch_add(1, Ordering::Relaxed);

    let placement = Signal::derive(move || placement().unwrap_or(UiPlacement::Top));

    Effect::new(move |prev| {
        let x = x();
        let y = y();
        let width = width();
        let height = height();
        let is_inside = !is_outside();

        if is_inside && prev != Some(true) {
            // add new entry
            let entry = TooltipEntry {
                placement,
                content,
                ui_rect: RwSignal::new((
                    mint::Point2 { x, y },
                    mint::Vector2 {
                        x: width,
                        y: height,
                    },
                )),
            };
            visible_tooltips.update(|d| {
                d.insert(id, entry);
            });
        } else if is_inside {
            // update exisiting position
            if let Some(entry) = visible_tooltips.get_untracked().get(&id) {
                entry.ui_rect.set((
                    mint::Point2 { x, y },
                    mint::Vector2 {
                        x: width,
                        y: height,
                    },
                ));
            }
        } else if prev == Some(true) {
            // hide entry
            visible_tooltips.update(|d| {
                d.remove(&id);
            });
        }

        is_inside
    });

    on_cleanup(move || {
        visible_tooltips.update(|d| {
            d.remove(&id);
        });
    });
}

#[component]
pub fn Tooltips() -> impl IntoView {
    let TooltipsContext(visible_tooltips) = expect_context();

    view! {
        <aside class="absolute w-screen h-screen top-0 left-0 pointer-events-none z-[150]">
            <For
                each=move || visible_tooltips().into_iter()
                key=|(id, _)| *id
                let((_, TooltipEntry { content, placement, ui_rect }))
            >
                <Tooltip text=content placement=placement rect=ui_rect.read_only() />
            </For>
        </aside>
    }
}
