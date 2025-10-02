use leptos::prelude::*;
use tauri_use::{use_invoke, UseTauriReturn};

use crate::{
    components::{Icon, Switch, UiPlacement, UiSize},
    util::tauri_resource::{use_tauri_resource, UseTauriResourceReturn},
};

#[component]
pub fn ActivationSourceToggle(
    #[prop(into, optional)] placement: Signal<Option<UiPlacement>>,
) -> impl IntoView {
    let UseTauriResourceReturn {
        data: activation_source,
        ..
    } = use_tauri_resource::<common::instrument::events::ActivationSourcePayload>(
        common::instrument::events::ACTIVATION_SRC,
    );

    let UseTauriReturn {
        error: set_activation_source_error,
        trigger: trigger_set_activation_source,
        ..
    } = use_invoke::<common::commands::instrument::ActivationSourcePayload, (), ()>(
        common::instrument::commands::SET_ACTIVATION_SRC,
    );

    let on_activation_source_change = Callback::new(move |source: usize| {
        trigger_set_activation_source(Some((
            common::instrument::commands::ActivationSourcePayload {
                source: source as u8,
            },
            (),
        )));
    });

    Effect::new(move || {
        if let Some(err) = set_activation_source_error() {
            log::error!(
                "Error invoking {}: {err}",
                common::instrument::commands::SET_ACTIVATION_SRC
            );
        }
    });

    view! {
        <Switch
            labels=vec![
                view! { <Icon name="entropy" size=UiSize::Sm /> }.into_any(),
                view! { <Icon name="mic" size=UiSize::Sm /> }.into_any(),
            ]
            tooltips=vec!["Random".to_string(), "Mic".to_string()]
            current_state=Signal::derive(move || {
                activation_source().map(|s| s.source).unwrap_or_default() as usize
            })
            on_change=on_activation_source_change
            size=UiSize::Sm
            placement=placement
        />
    }
}
