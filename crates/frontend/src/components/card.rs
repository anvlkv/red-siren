use leptos::prelude::*;
use leptos_styling::style_sheet;

use crate::components::{UiPadding, UiPlacement, UiVariant};

style_sheet!(
    card_animations,
    "src/components/card.css",
    "card_animations"
);

/// Generic Card component (MAYA DRY KISS).
///
/// Features:
/// - Variants: Solid | Outline | Ghost
/// - Padding: None | Sm | Md | Lg
/// - Rounded corners by default (can toggle)
/// - Optional interactive state (hover/active transitions; on_click)
/// - Full-width toggle
#[component]
pub fn Card(
    // Main content
    children: Children,

    // Appearance
    #[prop(optional, into)] variant: UiVariant,
    #[prop(optional, into)] padding: UiPadding,
    #[prop(optional, into)] rounded: bool,
    #[prop(optional, into)] full_width: bool,
    #[prop(optional, into)] class: Signal<String>,

    // Behavior
    #[prop(optional)] interactive: bool,

    // Animation
    #[prop(optional)] first_appear: bool,
    #[prop(optional, into)] card_animation_direction: Signal<Option<UiPlacement>>,
) -> impl IntoView {
    // Base layout and typography colors tuned to the existing theme
    let base = format!(
        "max-h-svh max-w-svw overflow-auto text-black dark:text-red \
                transition-all duration-200 \
                focus:outline-none focus-visible:ring-2 focus-visible:ring-offset-2 \
                relative preserve-3d will-change-transform {}",
        card_animations::CARD_CARD
    );

    let bg_and_border = match variant {
        UiVariant::Solid => {
            "\
             bg-linear-to-b from-cinnabar/60 to-red/60 dark:from-gray/60 dark:to-black/60 bg-red/40 dark:bg-black/40 bg-blend-screen dark:bg-blend-darken \
            shadow-xl shadow-gray dark:shadow-cinnabar backdrop-blur-sm"
        }
        UiVariant::Outline => {
            "\
            bg-transparent border-2 border-black dark:border-red \
            hover:shadow-sm hover:shadow-gray dark:hover:shadow-cinnabar"
        }
        UiVariant::Ghost => {
            "\
            bg-transparent"
        }
    };

    let rounding = if rounded { "rounded-xl" } else { "rounded-lg" };

    let padding_cls = padding.tw_class();

    let width_cls = if full_width { "w-full" } else { "w-max" };

    let interactive_cls = if interactive {
        "\
        cursor-pointer \
        hover:scale-[1.01] \
        active:scale-[0.995]"
    } else {
        "cursor-default"
    };

    let card_animation_class = move || {
        if let Some(dir) = card_animation_direction() {
            match dir {
                UiPlacement::Bottom => style_sheet_generated::ClassName::CARD_CARD_FROM_BOTTOM,
                UiPlacement::Top => style_sheet_generated::ClassName::CARD_CARD_FROM_TOP,
                UiPlacement::Left => style_sheet_generated::ClassName::CARD_CARD_FROM_LEFT,
                UiPlacement::Right => style_sheet_generated::ClassName::CARD_CARD_FROM_RIGHT,
            }
            .to_string()
        } else {
            Default::default()
        }
    };

    let class = Signal::derive(move || {
        let first_class = if first_appear {
            style_sheet_generated::ClassName::CARD_CARD_FIRST_APPEAR.to_string()
        } else {
            Default::default()
        };
        format!(
            "{base} {bg_and_border} {rounding} {padding_cls} {width_cls} {interactive_cls} {} {} {}",
            class(),
            card_animation_class(),
            first_class
        )
    });

    view! {
        <div
            role=if interactive { "button" } else { "group" }
            tabindex=if interactive { Some("0") } else { None }
            class=card_animations::CARD_SCENE
        >
            <div class=class>
                <div class="card-body contents backface-hidden">{children()}</div>
            </div>
        </div>
    }
}
