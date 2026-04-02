use leptos::prelude::*;
use tauri_use::{use_command, use_invoke, UseTauriReturn, UseTauriWithReturn};

pub struct UseFinetunedValuesReturn {
    pub values: Signal<common::commands::edit::FineTunedValuesPayload>,
    pub set_values: WriteSignal<common::commands::edit::FineTunedValuesPayload>,
    pub refresh: Callback<()>,
    pub submit: Callback<()>,
}

pub fn use_finetuned_values() -> UseFinetunedValuesReturn {
    let (finetuned_values, set_finetuned_values) =
        signal(common::commands::edit::FineTunedValuesPayload::default());

    let UseTauriWithReturn {
        data: fetch_values_data,
        trigger: fetch_values,
        error: fetch_values_error,
        ..
    } = use_command::<common::commands::edit::FineTunedValuesPayload>(
        common::commands::edit::GET_FINETUNED_VALUES,
    );

    let UseTauriReturn {
        trigger: update_values,
        error: update_values_error,
        data: update_values_data,
        ..
    } = use_invoke::<
        common::commands::edit::FineTunedValuesPayload,
        (),
        common::commands::edit::FineTunedValuesPayload,
    >(common::commands::edit::EDIT_FINETUNED_VALUES);

    let fetch_values_on_mount = fetch_values.clone();
    Effect::new(move |_| {
        fetch_values_on_mount(Some(()));
    });

    Effect::new(move |_| {
        if let Some(values) = fetch_values_data() {
            set_finetuned_values.set(values);
        }
    });

    Effect::new(move |_| {
        if let Some(values) = update_values_data() {
            set_finetuned_values.set(values);
        }
    });

    Effect::new(move |_| {
        if let Some(err) = fetch_values_error.get() {
            log::error!("Error fetching fine-tuned values: {err}");
        }
        if let Some(err) = update_values_error.get() {
            log::error!("Error updating fine-tuned values: {err}");
        }
    });

    let refresh_trigger = fetch_values.clone();
    let refresh = Callback::new(move |_| {
        refresh_trigger(Some(()));
    });

    let submit = Callback::new({
        let finetuned_values = finetuned_values;
        move |_| {
            update_values(Some((finetuned_values(), ())));
        }
    });

    UseFinetunedValuesReturn {
        values: finetuned_values.into(),
        set_values: set_finetuned_values,
        refresh,
        submit,
    }
}
