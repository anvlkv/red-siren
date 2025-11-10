use std::f32;

use leptos::prelude::*;
use tauri_use::{use_command, use_invoke, UseTauriReturn, UseTauriWithReturn};

use crate::components::{Button, EditorRangeSlider, Fold};

#[component]
pub fn EditorOverlay() -> impl IntoView {
    // Local state for form values (same payload as instrument editor)
    let (finetuned_values, set_finetuned_values) =
        signal(common::commands::edit::FineTunedValuesPayload::default());

    // Fetch current values (same command as instrument editor)
    let UseTauriWithReturn {
        data: fetch_values_data,
        trigger: fetch_values,
        error: fetch_values_error,
        ..
    } = use_command::<common::commands::edit::FineTunedValuesPayload>(
        common::commands::edit::GET_FINETUNED_VALUES,
    );

    // Update values command (same command/payloads as instrument editor)
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

    // Load current values on mount
    Effect::new(move |_| {
        fetch_values(Some(()));
    });

    // Update local state when values are fetched
    Effect::new(move |_| {
        if let Some(values) = fetch_values_data() {
            set_finetuned_values(values)
        }
    });

    // Update local state when values are updated
    Effect::new(move |_| {
        if let Some(values) = update_values_data() {
            set_finetuned_values(values)
        }
    });

    // Handle errors
    Effect::new(move |_| {
        if let Some(err) = fetch_values_error.get() {
            log::error!("Error fetching fine-tuned values: {err}");
        }
        if let Some(err) = update_values_error.get() {
            log::error!("Error updating fine-tuned values: {err}");
        }
    });

    // Submit handler
    let on_submit = move |ev: web_sys::SubmitEvent| {
        ev.prevent_default();
        update_values(Some((finetuned_values(), ())));
    };

    view! {
        <div class="absolute right-0 bottom-8 z-60 rounded bg-red/70 dark:bg-black/60 border border-black/20 dark:border-red/20 p-4 shadow-sm">
            <Fold title="Fine-tune Audio Parameters">
                <form on:submit=on_submit class="flex flex-wrap gap-4">
                    <fieldset class="flex flex-col gap-2">
                        <legend>"Input NY Compression"</legend>

                        <EditorRangeSlider
                            label="Threshold"
                            value=Signal::derive(move || { finetuned_values().input_ny_threshold })
                            min=f32::EPSILON
                            max=1.0
                            step=0.00001
                            on_input=Callback::new(move |val| {
                                set_finetuned_values
                                    .update(|values| {
                                        values.input_ny_threshold = val;
                                    });
                            })
                        />

                        <EditorRangeSlider
                            label="Wet Ratio"
                            value=Signal::derive(move || { finetuned_values().input_ny_wet_ratio })
                            min=0.0
                            max=1.0
                            step=0.01
                            on_input=Callback::new(move |val| {
                                set_finetuned_values
                                    .update(|values| {
                                        values.input_ny_wet_ratio = val;
                                    });
                            })
                        />
                    </fieldset>

                    <div class="flex gap-4 w-full justify-stretch">
                        <Button attr:r#type="submit">"Apply Changes"</Button>
                        <Button attr:r#type="button" on:click=move |_| fetch_values(Some(()))>
                            "Reset"
                        </Button>
                    </div>
                </form>
            </Fold>
        </div>
    }
}
