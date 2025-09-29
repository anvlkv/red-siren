use leptos::prelude::*;

use crate::components::{Tooltip, UiSize, UiVariant};

/// Multi-state segmented switch component for Red Siren (MAYA DRY KISS).
///
/// A segmented control that displays all labels at once with the selected state highlighted.
/// Each segment is clickable to directly set that state.
///
/// - Labels: Vec of views for each state
/// - Tooltips: optional Vec of tooltip strings for hover/focus
/// - Variants: Solid | Outline | Ghost (follows Button styling)
/// - Sizes: Sm | Md | Lg
///
/// Usage:
/// ```rust
/// view! {
///     <Switch
///         labels=vec![
///             view! { "Off" }.into_any(),
///             view! { "Auto" }.into_any(),
///             view! { "On" }.into_any(),
///         ]
///         tooltips=Some(vec!["Turn off".to_string(), "Automatic mode".to_string(), "Turn on".to_string()])
///         current_state=create_rw_signal(1)
///         on_change=Callback::new(|state| console_log!("State: {}", state))
///     />
/// }
/// ```
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
) -> impl IntoView {
    // Ensure at least 1 state
    if labels.is_empty() {
        panic!("Switch must have at least 1 label");
    }

    // Validate tooltips length if provided
    if let Some(ref tips) = tooltips {
        if tips.len() != labels.len() {
            panic!("Tooltips length must match labels length");
        }
    }

    let handle_segment_click = move |segment_index: usize| {
        move |_| {
            if !disabled.get_untracked() {
                on_change.run(segment_index);
            }
        }
    };

    // Base styles for the container
    let container_base = "inline-flex overflow-hidden";
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
            container_base,
            container_rounding(),
            container_variant(),
            class()
        )
    };

    // Base styles for individual segments
    let segment_base = "relative flex-1 flex items-center justify-center cursor-pointer \
                       transition-all duration-200 focus:outline-none \
                       focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-offset-0 \
                       italic";

    let segment_size = move || match size() {
        UiSize::Sm => "h-10 px-4 text-base",
        UiSize::Md => "h-12 px-5 text-lg",
        UiSize::Lg => "h-16 px-6 text-2xl",
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
                    "border-l border-black/20 dark:border-red/20"
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

        format!(
            "{} {} {} {} {}",
            segment_base,
            segment_size(),
            selected_cls,
            separator_cls,
            disabled_cls
        )
    };

    view! {
        <div class=container_class role="radiogroup">
            {labels
                .into_iter()
                .enumerate()
                .map(|(i, label)| {
                    let segment_index = i;
                    let is_selected = Signal::derive(move || current_state.get() == segment_index);
                    let segment_view = view! {
                        <button
                            type="button"
                            class=move || segment_class(segment_index, is_selected())
                            disabled=disabled
                            on:click=handle_segment_click(segment_index)
                            aria-pressed=is_selected
                        >
                            {label}
                        </button>
                    };
                    match &tooltips {
                        Some(tooltip_array) if segment_index < tooltip_array.len() => {
                            let tooltip_text = tooltip_array[segment_index].clone();

                            // Wrap with tooltip if tooltips are provided
                            view! { <Tooltip text=tooltip_text>{segment_view}</Tooltip> }
                                .into_any()
                        }
                        _ => segment_view.into_any(),
                    }
                })
                .collect_view()}
        </div>
    }
}
