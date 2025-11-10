use leptos::prelude::*;

use crate::components::{with_tooltip, UiPlacement, UiSize, UiVariant};

/// Multi-state segmented switch component for Red Siren (MAYA DRY KISS).
///
/// Refactored: orientation now inferred from shared UiPlacement (Left/Right => vertical, Top/Bottom => horizontal).
#[component]
pub fn Switch(
    // Label views for each state
    labels: Vec<AnyView>,

    // Optional tooltips for each state
    #[prop(optional)] tooltips: Option<Vec<String>>,

    // Current state (0 to labels.len()-1)
    #[prop(into)] current_state: Signal<usize>,

    // Callback when state changes
    #[prop(into)] on_change: Callback<usize>,

    // Styling props
    #[prop(optional, into)] disabled: Signal<bool>,
    #[prop(optional, into)] class: Signal<String>,
    #[prop(optional, into)] variant: Signal<UiVariant>,
    #[prop(optional, into)] size: Signal<UiSize>,
    #[prop(optional, into)] round: Signal<bool>,
    // Placement drives orientation (Top/Bottom horizontal, Left/Right vertical)
    #[prop(into)] placement: Signal<Option<UiPlacement>>,
) -> impl IntoView {
    if labels.len() < 2 {
        panic!("Switch must have at least 2 labels");
    }

    if let Some(ref tips) = tooltips {
        if tips.len() != labels.len() {
            panic!("Tooltips length must match labels length");
        }
    }

    let is_vertical = move || placement().unwrap_or_default().is_vertical();

    let handle_segment_click = move |segment_index: usize| {
        move |_| {
            if !disabled.get_untracked() {
                let current = current_state.get_untracked();
                if current != segment_index {
                    on_change.run(segment_index);
                }
            }
        }
    };

    let container_base = move || {
        if is_vertical() {
            "inline-flex flex-col overflow-visible"
        } else {
            "inline-flex overflow-visible"
        }
    };
    let container_rounding = move || {
        if round() {
            "rounded-full"
        } else {
            "rounded-lg"
        }
    };
    let container_variant = move || match variant() {
        UiVariant::Solid | UiVariant::Outline => "border-2 border-black dark:border-red",
        UiVariant::Ghost => "",
    };

    let container_class = move || {
        format!(
            "{} {} {} {}",
            container_base(),
            container_rounding(),
            container_variant(),
            class()
        )
    };

    // Removed flex-1 from base; horizontal growth now applied conditionally in segment_class so vertical stays square.
    let segment_base = "relative flex items-center justify-center cursor-pointer \
                       transition-all duration-200 focus:outline-none \
                       focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-offset-0 \
                       italic";

    let segment_size = move || {
        if is_vertical() {
            // Square sizing to match Button square variants
            match size() {
                UiSize::Sm => "md:h-10 md:w-10 h-8 w-8 md:text-base text-sm",
                UiSize::Md => "md:h-12 md:w-12 h-9 w-9 md:text-lg text-base",
                UiSize::Lg => "md:h-16 md:w-16 h-10 w-10 md:text-2xl text-lg",
            }
        } else {
            match size() {
                UiSize::Sm => "md:h-10 h-8 md:px-4 px-2 md:text-base text-sm",
                UiSize::Md => "md:h-12 h-9 md:px-5 px-3 md:text-lg text-base",
                UiSize::Lg => "md:h-16 h-10 md:px-6 px-4 md:text-2xl text-lg",
            }
        }
    };

    let segment_class = move |index: usize, is_selected: bool| {
        let selected_cls = if is_selected {
            match variant() {
                UiVariant::Solid => "bg-black dark:bg-red text-red dark:text-black shadow-inner",
                UiVariant::Outline => "bg-black/10 dark:bg-red/10 text-black dark:text-red",
                UiVariant::Ghost => "bg-black/20 dark:bg-red/20 text-black dark:text-red",
            }
        } else {
            match variant() {
                UiVariant::Solid => "text-black dark:text-red hover:bg-black/5 dark:hover:bg-red/5",
                UiVariant::Outline => {
                    "text-black dark:text-red hover:bg-black/5 dark:hover:bg-red/5"
                }
                UiVariant::Ghost => {
                    "text-black dark:text-red hover:bg-black/10 dark:hover:bg-red/10"
                }
            }
        };

        let separator_cls = if index > 0 && !is_selected {
            match variant() {
                UiVariant::Solid | UiVariant::Outline => {
                    if is_vertical() {
                        "border-t border-black/20 dark:border-red/20"
                    } else {
                        "border-l border-black/20 dark:border-red/20"
                    }
                }
                UiVariant::Ghost => "",
            }
        } else {
            ""
        };

        let disabled_cls = if disabled() {
            "opacity-50 pointer-events-none"
        } else {
            ""
        };
        // Horizontal segments expand; vertical stay fixed-size squares.
        let grow_cls = if !is_vertical() { "flex-1" } else { "" };

        format!(
            "{} {} {} {} {} {}",
            segment_base,
            grow_cls,
            segment_size(),
            selected_cls,
            separator_cls,
            disabled_cls
        )
    };

    let tooltip_placement = Signal::derive(move || placement().map(|p| p.opposite()));

    view! {
        <div
            class=container_class
            role="radiogroup"
            aria-orientation=move || if is_vertical() { "vertical" } else { "horizontal" }
        >
            {labels
                .into_iter()
                .enumerate()
                .map(|(i, label)| {
                    let segment_index = i;
                    let is_selected = Signal::derive(move || current_state.get() == segment_index);
                    let button_ref = NodeRef::new();
                    if let Some(tooltip_array) = &tooltips {
                        let tooltip_text = tooltip_array[segment_index].clone();
                        with_tooltip(
                            button_ref,
                            Signal::derive(move || tooltip_text.clone()),
                            tooltip_placement,
                        );
                    }

                    view! {
                        <button
                            type="button"
                            class=move || segment_class(segment_index, is_selected())
                            disabled=disabled
                            on:click=handle_segment_click(segment_index)
                            aria-pressed=is_selected
                            node_ref=button_ref
                        >
                            {label}
                        </button>
                    }
                })
                .collect_view()}
        </div>
    }
}
