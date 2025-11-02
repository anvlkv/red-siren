use leptos::prelude::*;
use tauri_use::{use_command, use_invoke, UseTauriReturn, UseTauriWithReturn};

use crate::components::{Button, EditorRangeSlider, Fold};

const FREQ_RATIO_MAX: f32 = 14.0;
const FREQ_RATIO_MIN: f32 = 0.001;
const FREQ_RATIO_STEP: f32 = 0.001;

const Q_MAX: f32 = 10.0;
const Q_MIN: f32 = 0.001;
const Q_STEP: f32 = 0.001;

const GAIN_MAX: f32 = 24.0;
const GAIN_MIN: f32 = -24.0;
const GAIN_STEP: f32 = 0.1;

const TIME_MAX: f32 = 10.0;
const TIME_MIN: f32 = 0.01;
const TIME_STEP: f32 = 0.01;

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
                            min=TIME_MIN
                            max=TIME_MAX
                            step=TIME_STEP
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
                            min=TIME_MIN
                            max=TIME_MAX
                            step=TIME_STEP
                            on_input=Callback::new(move |val| {
                                set_finetuned_values
                                    .update(|values| {
                                        values.filter_switch_follow_response_s = val;
                                    });
                            })
                        />

                        <div class="flex gap-2">
                            <fieldset class="flex flex-col gap-2">
                                <legend>"Active key"</legend>
                                <EditorRangeSlider
                                    label="Allpass Q"
                                    value=Signal::derive(move || {
                                        finetuned_values().filter_allpass_q
                                    })
                                    min=Q_MIN
                                    max=Q_MAX
                                    step=Q_STEP
                                    on_input=Callback::new(move |val| {
                                        set_finetuned_values
                                            .update(|values| {
                                                values.filter_allpass_q = val;
                                            });
                                    })
                                />

                                <EditorRangeSlider
                                    label="Moog Q"
                                    value=Signal::derive(move || {
                                        finetuned_values().filter_moog_q
                                    })
                                    min=Q_MIN
                                    max=Q_MAX
                                    step=Q_STEP
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
                                    min=Q_MIN
                                    max=Q_MAX
                                    step=Q_STEP
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
                                    min=FREQ_RATIO_MIN
                                    max=FREQ_RATIO_MAX
                                    step=FREQ_RATIO_STEP
                                    on_input=Callback::new(move |val| {
                                        set_finetuned_values
                                            .update(|values| {
                                                values.filter_moog_freq_ratio = val;
                                            });
                                    })
                                />
                            </fieldset>
                            <fieldset>
                                <legend>"Plain key"</legend>
                                <EditorRangeSlider
                                    label="Shelf freq ratio"
                                    value=Signal::derive(move || {
                                        finetuned_values().filter_shelf_freq_ratio
                                    })
                                    min=FREQ_RATIO_MIN
                                    max=FREQ_RATIO_MAX
                                    step=FREQ_RATIO_STEP
                                    on_input=Callback::new(move |val| {
                                        set_finetuned_values
                                            .update(|values| {
                                                values.filter_shelf_freq_ratio = val;
                                            });
                                    })
                                />

                                <EditorRangeSlider
                                    label="Shelf Q"
                                    value=Signal::derive(move || {
                                        finetuned_values().filter_shelf_q
                                    })
                                    min=Q_MIN
                                    max=Q_MAX
                                    step=Q_STEP
                                    on_input=Callback::new(move |val| {
                                        set_finetuned_values
                                            .update(|values| {
                                                values.filter_shelf_q = val;
                                            });
                                    })
                                />

                                <EditorRangeSlider
                                    label="Shelf Gain dB"
                                    value=Signal::derive(move || {
                                        finetuned_values().filter_shelf_gain
                                    })
                                    min=GAIN_MIN
                                    max=GAIN_MAX
                                    step=GAIN_STEP
                                    on_input=Callback::new(move |val| {
                                        set_finetuned_values
                                            .update(|values| {
                                                values.filter_shelf_gain = val;
                                            });
                                    })
                                />

                                <EditorRangeSlider
                                    label="Pass freq ratio"
                                    value=Signal::derive(move || {
                                        finetuned_values().filter_pass_freq_ratio
                                    })
                                    min=FREQ_RATIO_MIN
                                    max=FREQ_RATIO_MAX
                                    step=FREQ_RATIO_STEP
                                    on_input=Callback::new(move |val| {
                                        set_finetuned_values
                                            .update(|values| {
                                                values.filter_pass_freq_ratio = val;
                                            });
                                    })
                                />

                                <EditorRangeSlider
                                    label="Pass Q"
                                    value=Signal::derive(move || {
                                        finetuned_values().filter_pass_q
                                    })
                                    min=Q_MIN
                                    max=Q_MAX
                                    step=Q_STEP
                                    on_input=Callback::new(move |val| {
                                        set_finetuned_values
                                            .update(|values| {
                                                values.filter_pass_q = val;
                                            });
                                    })
                                />
                            </fieldset>
                        </div>

                    </fieldset>

                    <fieldset class="flex flex-col gap-2">
                        <legend>"Node Parameters"</legend>

                        <EditorRangeSlider
                            label="Follow Response Time"
                            value=Signal::derive(move || {
                                finetuned_values().node_follow_response_time_s
                            })
                            min=TIME_MIN
                            max=TIME_MAX
                            step=TIME_STEP
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
                            min=Q_MIN
                            max=Q_MAX
                            step=Q_STEP
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
                            min=GAIN_MIN
                            max=GAIN_MAX
                            step=GAIN_STEP
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
                            min=Q_MIN
                            max=Q_MAX
                            step=Q_STEP
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
                            label="Ratio"
                            value=Signal::derive(move || { finetuned_values().input_ny_ratio })
                            min=0.001
                            max=10.0
                            step=0.001
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
