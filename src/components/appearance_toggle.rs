use leptos::prelude::*;
use tauri_use::{use_invoke_with_args, UseTauriWithReturn};

use crate::{
    components::{Icon, Switch, UiPlacement, UiSize},
    util::tauri_resource::{use_tauri_resource, UseTauriResourceReturn},
};

#[component]
pub fn AppearanceToggle(
    #[prop(into, optional)] placement: Signal<Option<UiPlacement>>,
) -> impl IntoView {
    let UseTauriResourceReturn {
        data: dark_override,
        ..
    } = use_tauri_resource::<common::commands::setup::UpdateWindowAppearanceOverridePayload>(
        common::commands::setup::GET_WINDOW_APPEARANCE_OVERRIDE,
    );

    let dark_override_value = Signal::derive(move || {
        dark_override
            .get()
            .map(|d| match d.dark {
                Some(true) => 2_usize,
                Some(false) => 1_usize,
                None => 0_usize,
            })
            .unwrap_or(0)
    });

    let UseTauriWithReturn {
        error: appearance_override_error,
        trigger: trigger_appearance_override,
        ..
    } = use_invoke_with_args::<common::commands::setup::UpdateWindowAppearanceOverridePayload, ()>(
        common::commands::setup::WINDOW_APPEARANCE_OVERRIDE,
    );

    let on_appearance_override_change = Callback::new(move |source: usize| {
        let dark = match source {
            0 => None,
            1 => Some(false),
            2 => Some(true),
            _ => None,
        };

        log::debug!("Setting appearance override to: {:?}", dark);

        trigger_appearance_override(Some(
            common::commands::setup::UpdateWindowAppearanceOverridePayload { dark },
        ));
    });

    Effect::new(move || {
        if let Some(err) = appearance_override_error() {
            log::error!(
                "Error invoking {}: {err}",
                common::commands::setup::WINDOW_APPEARANCE_OVERRIDE
            );
        }
    });

    view! {
        <Switch
            labels=vec![
                view! { <Icon name="system" size=UiSize::Sm /> }.into_any(),
                view! { <Icon name="bright" size=UiSize::Sm /> }.into_any(),
                view! { <Icon name="dark" size=UiSize::Sm /> }.into_any(),
            ]
            tooltips=vec![
                "System theme (auto)".to_string(),
                "In (bright)".to_string(),
                "Yo (dark)".to_string(),
            ]
            current_state=dark_override_value
            on_change=on_appearance_override_change
            size=UiSize::Sm
            placement=placement
        />
    }
}
