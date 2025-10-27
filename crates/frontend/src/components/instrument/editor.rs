use leptos::prelude::*;
use tauri_use::{use_command, use_invoke, UseTauriReturn, UseTauriWithReturn};

use crate::components::{Button, Fold};

#[component]
pub fn EditorOverlay() -> impl IntoView {
    // Local state for form values
    let (finetuned_values, set_finetuned_values) =
        signal(common::commands::edit::FineTunedValuesPayload::default());

    // Fetch current values
    let UseTauriWithReturn {
        data: fetch_values_data,
        trigger: fetch_values,
        error: fetch_values_error,
        ..
    } = use_command::<common::commands::edit::FineTunedValuesPayload>(
        common::commands::edit::GET_FINETUNED_VALUES,
    );

    // Update values command
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
                        <legend>"Siren Parameters"</legend>

                        <EditorRangeSlider
                            value=Signal::derive(move || { finetuned_values().siren_base_hz })
                            on_input=Callback::new(move |val| {
                                set_finetuned_values
                                    .update(|values| {
                                        values.siren_base_hz = val;
                                    });
                            })
                            min=0.1
                            max=2.0
                            step=0.01
                            label="Base Hz"
                        />

                        <EditorRangeSlider
                            label="Max Frequency Hz"
                            value=Signal::derive(move || {
                                finetuned_values().siren_max_frequency_hz
                            })
                            min=100.0
                            max=15000.0
                            step=1.0
                            on_input=Callback::new(move |val| {
                                set_finetuned_values
                                    .update(|values| {
                                        values.siren_max_frequency_hz = val;
                                    });
                            })
                        />

                        <EditorRangeSlider
                            label="Excitement Pause Limit"
                            value=Signal::derive(move || {
                                finetuned_values().siren_excitement_pause_limit
                            })
                            min=0.1
                            max=1.0
                            step=0.01
                            on_input=Callback::new(move |val| {
                                set_finetuned_values
                                    .update(|values| {
                                        values.siren_excitement_pause_limit = val;
                                    });
                            })
                        />

                        <EditorRangeSlider
                            label="Base Pause Duration"
                            value=Signal::derive(move || {
                                finetuned_values().siren_base_pause_duration
                            })
                            min=0.01
                            max=1.0
                            step=0.01
                            on_input=Callback::new(move |val| {
                                set_finetuned_values
                                    .update(|values| {
                                        values.siren_base_pause_duration = val;
                                    });
                            })
                        />
                    </fieldset>

                    <fieldset class="flex flex-col gap-2">
                        <legend>"Filter Parameters"</legend>

                        <EditorRangeSlider
                            label="Switch Follow Response"
                            value=Signal::derive(move || {
                                finetuned_values().filter_switch_follow_response_s
                            })
                            min=0.001
                            max=0.2
                            step=0.001
                            on_input=Callback::new(move |val| {
                                set_finetuned_values
                                    .update(|values| {
                                        values.filter_switch_follow_response_s = val;
                                    });
                            })
                        />

                        <EditorRangeSlider
                            label="Allpass Q"
                            value=Signal::derive(move || { finetuned_values().filter_allpass_q })
                            min=0.1
                            max=2.0
                            step=0.01
                            on_input=Callback::new(move |val| {
                                set_finetuned_values
                                    .update(|values| {
                                        values.filter_allpass_q = val;
                                    });
                            })
                        />

                        <EditorRangeSlider
                            label="Moog Q"
                            value=Signal::derive(move || { finetuned_values().filter_moog_q })
                            min=0.1
                            max=2.0
                            step=0.01
                            on_input=Callback::new(move |val| {
                                set_finetuned_values
                                    .update(|values| {
                                        values.filter_moog_q = val;
                                    });
                            })
                        />

                        <EditorRangeSlider
                            label="Allpass freq ratio"
                            value=Signal::derive(move || {
                                finetuned_values().filter_allpass_freq_ratio
                            })
                            min=0.1
                            max=2.0
                            step=0.01
                            on_input=Callback::new(move |val| {
                                set_finetuned_values
                                    .update(|values| {
                                        values.filter_allpass_freq_ratio = val;
                                    });
                            })
                        />

                        <EditorRangeSlider
                            label="Moog freq ratio"
                            value=Signal::derive(move || {
                                finetuned_values().filter_moog_freq_ratio
                            })
                            min=0.1
                            max=2.0
                            step=0.01
                            on_input=Callback::new(move |val| {
                                set_finetuned_values
                                    .update(|values| {
                                        values.filter_moog_freq_ratio = val;
                                    });
                            })
                        />
                    </fieldset>

                    <fieldset class="flex flex-col gap-2">
                        <legend>"Node Parameters"</legend>

                        <EditorRangeSlider
                            label="Follow Response Time"
                            value=Signal::derive(move || {
                                finetuned_values().node_follow_response_time_s
                            })
                            min=0.01
                            max=1.0
                            step=0.001
                            on_input=Callback::new(move |val| {
                                set_finetuned_values
                                    .update(|values| {
                                        values.node_follow_response_time_s = val;
                                    });
                            })
                        />

                        <EditorRangeSlider
                            label="Bell Q"
                            value=Signal::derive(move || { finetuned_values().node_bell_q })
                            min=0.01
                            max=1.5
                            step=0.001
                            on_input=Callback::new(move |val| {
                                set_finetuned_values
                                    .update(|values| {
                                        values.node_bell_q = val;
                                    });
                            })
                        />

                        <EditorRangeSlider
                            label="Bell Gain dB"
                            value=Signal::derive(move || { finetuned_values().node_bell_gain_db })
                            min=0.0
                            max=10.0
                            step=0.1
                            on_input=Callback::new(move |val| {
                                set_finetuned_values
                                    .update(|values| {
                                        values.node_bell_gain_db = val;
                                    });
                            })
                        />
                    </fieldset>

                    <fieldset class="flex flex-col gap-2">
                        <legend>"Formant Parameters"</legend>

                        <EditorRangeSlider
                            label="Base Q"
                            value=Signal::derive(move || { finetuned_values().formant_base_q })
                            min=0.1
                            max=2.0
                            step=0.01
                            on_input=Callback::new(move |val| {
                                set_finetuned_values
                                    .update(|values| {
                                        values.formant_base_q = val;
                                    });
                            })
                        />
                    </fieldset>

                    <fieldset class="flex flex-col gap-2">
                        <legend>"Input NY Compression"</legend>

                        <EditorRangeSlider
                            label="Threshold"
                            value=Signal::derive(move || { finetuned_values().input_ny_threshold })
                            min=0.0
                            max=1.0
                            step=0.01
                            on_input=Callback::new(move |val| {
                                set_finetuned_values
                                    .update(|values| {
                                        values.input_ny_threshold = val;
                                    });
                            })
                        />

                        <EditorRangeSlider
                            label="Ratio"
                            value=Signal::derive(move || { finetuned_values().input_ny_ratio })
                            min=1.0
                            max=10.0
                            step=0.1
                            on_input=Callback::new(move |val| {
                                set_finetuned_values
                                    .update(|values| {
                                        values.input_ny_ratio = val;
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

#[component]
fn EditorRangeSlider(
    label: &'static str,
    value: Signal<f32>,
    min: f32,
    max: f32,
    step: f32,
    on_input: Callback<f32>,
) -> impl IntoView {
    view! {
        <label class="flex flex-col gap-1">
            <span class="block">{move || format!("{label}: {:.3}", value())}</span>
            <input
                type="range"
                min=min.to_string()
                max=max.to_string()
                step=step.to_string()
                prop:value=value
                on:input=move |ev| {
                    if let Ok(val) = event_target_value(&ev).parse::<f32>() {
                        on_input.run(val);
                    }
                }
            />
        </label>
    }
}
