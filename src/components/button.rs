use std::str::FromStr;

use leptos::prelude::*;

/// Generic, theme-aware button for Red Siren (MAYA DRY KISS).
///
/// - Variants: Solid | Outline | Ghost
/// - Sizes: Sm | Md | Lg
/// - Shapes: rounded (default) or fully round (circle) via `round`
/// - Square: `square=true` makes width equal height (great for icon-only)
/// - Full width: `full_width=true` stretches to container width
/// - Disabled: `disabled=true` applies opacity and blocks pointer events
///
/// Usage:
/// view! {
///   <Button on_click=Some(Callback::new(move |_| do_something()))>"Click me"</Button>
///   <Button variant=ButtonVariant::Outline size=ButtonSize::Sm class=Some("mt-2".into())>
///     "Secondary"
///   </Button>
///   <Button round square size=ButtonSize::Lg aria_label=Some("Info".into())>
///     <crate::components::Icon name="info" stroke_width=12.0 />
///   </Button>
/// }
#[component]
pub fn Button(
    // Content inside the button
    children: Children,

    #[prop(optional, into)] disabled: bool,
    #[prop(optional, into)] class: String,

    // Appearance
    #[prop(optional, into)] variant: ButtonVariant,
    #[prop(optional, into)] size: ButtonSize,
    #[prop(optional, into)] round: bool,
    #[prop(optional, into)] square: bool,
    #[prop(optional, into)] full_width: bool,
) -> impl IntoView {
    // Compose classes (tailwind-like)
    let base = "relative inline-flex items-center justify-center cursor-pointer \
                transition-colors transition-shadow transition-opacity duration-200 focus:outline-none \
                focus-visible:ring-2 focus-visible:ring-offset-2 \
                hover:shadow-md active:shadow-sm italic";
    let rounding = if round { "rounded-full" } else { "rounded-lg" };

    let size_cls = if square {
        match size {
            ButtonSize::Sm => "h-10 w-10 text-xl",
            ButtonSize::Md => "h-12 w-12 text-2xl",
            ButtonSize::Lg => "h-16 w-16 text-4xl",
        }
    } else {
        match size {
            ButtonSize::Sm => "h-10 px-4 text-base",
            ButtonSize::Md => "h-12 px-5 text-lg",
            ButtonSize::Lg => "h-16 px-6 text-2xl",
        }
    };

    let variant_cls = match variant {
        ButtonVariant::Solid => {
            "\
            bg-black dark:bg-red \
            text-red dark:text-black \
            hover:shadow-gray dark:hover:shadow-cinnabar \
            active:shadow-gray dark:active:shadow-cinnabar"
        }
        ButtonVariant::Outline => {
            "\
            border-2 border-black dark:border-red \
            text-black dark:text-red \
            hover:bg-black/5 dark:hover:bg-red/10 \
            hover:shadow-gray dark:hover:shadow-cinnabar \
            active:shadow-gray dark:active:shadow-cinnabar"
        }
        ButtonVariant::Ghost => {
            "\
            text-black dark:text-red \
            hover:bg-black/10 dark:hover:bg-red/10 \
            hover:shadow-gray dark:hover:shadow-cinnabar \
            active:shadow-gray dark:active:shadow-cinnabar"
        }
    };

    let width_cls = if full_width { "w-full" } else { "w-auto" };

    let disabled_cls = if disabled {
        "opacity-50 pointer-events-none"
    } else {
        ""
    };

    let class =
        format!("{base} {rounding} {size_cls} {variant_cls} {width_cls} {disabled_cls} {class}");

    view! {
        <button type="button" class=class disabled=disabled>
            {children()}
        </button>
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, strum::EnumString)]
pub enum ButtonVariant {
    #[default]
    Solid,
    Outline,
    Ghost,
}

impl From<String> for ButtonVariant {
    fn from(value: String) -> Self {
        Self::from_str(&value).expect("invalid button variant")
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, strum::EnumString)]
pub enum ButtonSize {
    Sm,
    Md,
    #[default]
    Lg,
}

impl From<String> for ButtonSize {
    fn from(value: String) -> Self {
        Self::from_str(&value).expect("invalid button size")
    }
}
