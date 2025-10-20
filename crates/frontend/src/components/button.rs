use leptos::prelude::*;
use leptos_router::components::A;

use crate::components::{UiPlacement, UiSize, UiVariant};

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
///     <crate::components::Icon name="info" />
///   </Button>
/// }
#[component]
pub fn Button(
    // Content inside the button
    children: ChildrenFn,

    #[prop(optional, into)] disabled: Signal<bool>,
    #[prop(optional, into)] class: Signal<String>,

    // Optional link; when provided renders <A> instead of <button>
    #[prop(optional, into)] href: Signal<Option<String>>,

    // Appearance
    #[prop(optional, into)] variant: Signal<UiVariant>,
    #[prop(optional, into)] size: Signal<UiSize>,
    #[prop(optional, into)] round: Signal<bool>,
    #[prop(optional, into)] square: Signal<bool>,

    // Optional placement for directional / edge-aware layout (Left/Right => vertical stacking)
    #[prop(optional, into)] placement: Signal<Option<UiPlacement>>,
) -> impl IntoView {
    let base = "relative inline-flex items-center justify-center cursor-pointer \
            transition-colors transition-shadow transition-opacity duration-200 focus:outline-none \
            focus-visible:ring-2 focus-visible:ring-offset-2 \
            hover:shadow-md active:shadow-sm italic backface-hidden antialiased";
    let class = move || {
        let rounding = if round() {
            "rounded-full"
        } else {
            "rounded-lg"
        };

        let size_cls = if square() {
            match size() {
                UiSize::Sm => "h-10 w-10 md:text-xl text-lg",
                UiSize::Md => "h-12 w-12 md:text-2xl text-xl",
                UiSize::Lg => "h-16 w-16 md:text-4xl text-2xl",
            }
        } else {
            match size() {
                UiSize::Sm => "h-10 px-4 md:text-base text-sm",
                UiSize::Md => "h-12 px-5 md:text-lg text-base",
                UiSize::Lg => "h-16 px-6 md:text-2xl text-lg",
            }
        };

        let variant_cls = match variant() {
            UiVariant::Solid => {
                "\
            bg-black dark:bg-red \
            text-red dark:text-black \
            hover:shadow-gray dark:hover:shadow-cinnabar \
            active:shadow-gray dark:active:shadow-cinnabar"
            }
            UiVariant::Outline => {
                "\
            border-2 border-black dark:border-red \
            text-black dark:text-red \
            hover:bg-black/5 dark:hover:bg-red/10 \
            hover:shadow-gray dark:hover:shadow-cinnabar \
            active:shadow-gray dark:active:shadow-cinnabar"
            }
            UiVariant::Ghost => {
                "\
            text-black dark:text-red \
            hover:bg-black/10 dark:hover:bg-red/10 \
            hover:shadow-gray dark:hover:shadow-cinnabar \
            active:shadow-gray dark:active:shadow-cinnabar"
            }
        };
        let disabled_cls = if disabled() {
            "opacity-50 pointer-events-none"
        } else {
            ""
        };

        format!(
            "{base} {rounding} {size_cls} {variant_cls} {disabled_cls} {}",
            class()
        )
    };

    let button_style = Signal::derive(move || {
        if matches!(placement(), Some(UiPlacement::Left | UiPlacement::Right)) {
            "writing-mode: vertical-rl; text-orientation: mixed; height: auto;"
        } else {
            ""
        }
    });

    view! {
        {move || {
            let children = children.clone();
            if let Some(href) = href() {
                view! {
                    <A href=href attr:class=class attr:style=button_style attr:r#type="button">
                        {children()}
                    </A>
                }
                    .into_any()
            } else {
                view! {
                    <button class=class disabled=disabled style=button_style>
                        {children()}
                    </button>
                }
                    .into_any()
            }
        }}
    }
}
