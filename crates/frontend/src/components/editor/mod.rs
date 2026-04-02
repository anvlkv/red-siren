mod finetuned_values;

pub use finetuned_values::{use_finetuned_values, UseFinetunedValuesReturn};

use common::commands::edit::FineTunedValuesPayload;
use leptos::prelude::*;

use super::{Button, Fold, RangeSlider, SliderValue};

const Q_MAX: f32 = 10.0;
const Q_MIN: f32 = 0.001;
const Q_STEP: f32 = 0.001;

const GAIN_MAX: f32 = 24.0;
const GAIN_MIN: f32 = -24.0;
const GAIN_STEP: f32 = 0.1;

const TIME_MAX: f32 = 10.0;
const TIME_MIN: f32 = 0.01;
const TIME_STEP: f32 = 0.01;

fn slider_input<F>(
    set_values: WriteSignal<FineTunedValuesPayload>,
    apply: F,
) -> Callback<SliderValue>
where
    F: Fn(&mut FineTunedValuesPayload, f32) + Send + Sync + 'static,
{
    Callback::new(move |val: SliderValue| {
        let value: f32 = val.into();
        set_values.update(|payload| apply(payload, value));
    })
}

/// Standalone editor surface for fine-tuning instrument parameters.
/// This is shared between the devtools page and any future inline surfaces.
#[component]
pub fn EditorPanel() -> impl IntoView {
    let UseFinetunedValuesReturn {
        values,
        set_values,
        refresh,
        submit,
    } = use_finetuned_values();

    let on_submit = move |ev: web_sys::SubmitEvent| {
        ev.prevent_default();
        submit.run(());
    };
    let refresh_callback = refresh;
    let on_reset = move |_: web_sys::MouseEvent| {
        refresh_callback.run(());
    };

    view! {
        <div class="w-full h-full overflow-auto px-4 md:px-8">
            <div class="max-w-5xl mx-auto space-y-6">
                <Fold title="Fine-tune Audio Parameters">
                    <form on:submit=on_submit class="p-4 w-full overflow-x-auto space-y-6">
                        <div class="flex flex-col gap-6 min-w-[900px]">
                            <fieldset class="flex flex-col gap-2">
                                <legend>"Siren Parameters"</legend>

                                <RangeSlider
                                    label="Alpha"
                                    value=Signal::derive(move || values().siren_alpha.into())
                                    min=0.1
                                    max=20000.0
                                    step=1.0
                                    on_input={
                                        let set_values = set_values;
                                        slider_input(
                                            set_values,
                                            |payload, value| {
                                                payload.siren_alpha = value;
                                            },
                                        )
                                    }
                                />
                            </fieldset>

                            <fieldset class="flex flex-col gap-4">
                                <legend>"Filter Parameters"</legend>

                                <RangeSlider
                                    label="Morph Follow Time"
                                    value=Signal::derive(move || {
                                        values().filter_morph_follow_s.into()
                                    })
                                    min=TIME_MIN
                                    max=TIME_MAX
                                    step=TIME_STEP
                                    on_input={
                                        let set_values = set_values;
                                        slider_input(
                                            set_values,
                                            |payload, value| {
                                                payload.filter_morph_follow_s = value;
                                            },
                                        )
                                    }
                                />

                                <div class="flex gap-4 flex-wrap">
                                    <fieldset class="flex-1 min-w-[260px] flex flex-col gap-2">
                                        <legend>"Active key"</legend>

                                        <RangeSlider
                                            label="Piercing Q"
                                            value=Signal::derive(move || {
                                                values().filter_q_piercing.into()
                                            })
                                            min=Q_MIN
                                            max=Q_MAX
                                            step=Q_STEP
                                            on_input={
                                                let set_values = set_values;
                                                slider_input(
                                                    set_values,
                                                    |payload, value| {
                                                        payload.filter_q_piercing = value;
                                                    },
                                                )
                                            }
                                        />

                                        <RangeSlider
                                            label="Bright Q"
                                            value=Signal::derive(move || {
                                                values().filter_q_bright.into()
                                            })
                                            min=Q_MIN
                                            max=Q_MAX
                                            step=Q_STEP
                                            on_input={
                                                let set_values = set_values;
                                                slider_input(
                                                    set_values,
                                                    |payload, value| {
                                                        payload.filter_q_bright = value;
                                                    },
                                                )
                                            }
                                        />
                                    </fieldset>

                                    <fieldset class="flex-1 min-w-[260px] flex flex-col gap-2">
                                        <legend>"Plain key"</legend>

                                        <RangeSlider
                                            label="Shelf Q"
                                            value=Signal::derive(move || values().filter_q_shelf.into())
                                            min=Q_MIN
                                            max=Q_MAX
                                            step=Q_STEP
                                            on_input={
                                                let set_values = set_values;
                                                slider_input(
                                                    set_values,
                                                    |payload, value| {
                                                        payload.filter_q_shelf = value;
                                                    },
                                                )
                                            }
                                        />

                                        <RangeSlider
                                            label="Shelf Gain dB"
                                            value=Signal::derive(move || {
                                                values().filter_shelf_gain_db.into()
                                            })
                                            min=GAIN_MIN
                                            max=GAIN_MAX
                                            step=GAIN_STEP
                                            on_input={
                                                let set_values = set_values;
                                                slider_input(
                                                    set_values,
                                                    |payload, value| {
                                                        payload.filter_shelf_gain_db = value;
                                                    },
                                                )
                                            }
                                        />

                                        <RangeSlider
                                            label="Warm Q"
                                            value=Signal::derive(move || values().filter_q_warm.into())
                                            min=Q_MIN
                                            max=Q_MAX
                                            step=Q_STEP
                                            on_input={
                                                let set_values = set_values;
                                                slider_input(
                                                    set_values,
                                                    |payload, value| {
                                                        payload.filter_q_warm = value;
                                                    },
                                                )
                                            }
                                        />
                                    </fieldset>
                                </div>
                            </fieldset>

                            <fieldset class="flex flex-col gap-2">
                                <legend>"Node Parameters"</legend>

                                <RangeSlider
                                    label="Follow Response Time"
                                    value=Signal::derive(move || {
                                        values().node_follow_response_time_s.into()
                                    })
                                    min=TIME_MIN
                                    max=TIME_MAX
                                    step=TIME_STEP
                                    on_input={
                                        let set_values = set_values;
                                        slider_input(
                                            set_values,
                                            |payload, value| {
                                                payload.node_follow_response_time_s = value;
                                            },
                                        )
                                    }
                                />

                                <RangeSlider
                                    label="Bell Q"
                                    value=Signal::derive(move || values().node_bell_q.into())
                                    min=Q_MIN
                                    max=Q_MAX
                                    step=Q_STEP
                                    on_input={
                                        let set_values = set_values;
                                        slider_input(
                                            set_values,
                                            |payload, value| {
                                                payload.node_bell_q = value;
                                            },
                                        )
                                    }
                                />

                                <RangeSlider
                                    label="Bell Gain dB"
                                    value=Signal::derive(move || values().node_bell_gain_db.into())
                                    min=GAIN_MIN
                                    max=GAIN_MAX
                                    step=GAIN_STEP
                                    on_input={
                                        let set_values = set_values;
                                        slider_input(
                                            set_values,
                                            |payload, value| {
                                                payload.node_bell_gain_db = value;
                                            },
                                        )
                                    }
                                />
                            </fieldset>

                            <fieldset class="flex flex-col gap-2">
                                <legend>"Group Parameters"</legend>

                                <RangeSlider
                                    label="Group Q"
                                    value=Signal::derive(move || values().group_q.into())
                                    min=Q_MIN
                                    max=Q_MAX
                                    step=Q_STEP
                                    on_input={
                                        let set_values = set_values;
                                        slider_input(
                                            set_values,
                                            |payload, value| {
                                                payload.group_q = value;
                                            },
                                        )
                                    }
                                />

                                <RangeSlider
                                    label="Group Low Shelf Gain dB"
                                    value=Signal::derive(move || values().group_ls_gain_db.into())
                                    min=GAIN_MIN
                                    max=GAIN_MAX
                                    step=GAIN_STEP
                                    on_input={
                                        let set_values = set_values;
                                        slider_input(
                                            set_values,
                                            |payload, value| {
                                                payload.group_ls_gain_db = value;
                                            },
                                        )
                                    }
                                />
                            </fieldset>

                            <fieldset class="flex flex-col gap-2">
                                <legend>"Formant Parameters"</legend>

                                <RangeSlider
                                    label="Base Q"
                                    value=Signal::derive(move || values().formant_base_q.into())
                                    min=Q_MIN
                                    max=Q_MAX
                                    step=Q_STEP
                                    on_input={
                                        let set_values = set_values;
                                        slider_input(
                                            set_values,
                                            |payload, value| {
                                                payload.formant_base_q = value;
                                            },
                                        )
                                    }
                                />
                            </fieldset>
                        </div>

                        <div class="flex gap-4 w-full justify-stretch">
                            <Button attr:r#type="submit">"Apply Changes"</Button>
                            <Button attr:r#type="button" on:click=on_reset>
                                "Reset"
                            </Button>
                        </div>
                    </form>
                </Fold>
            </div>
        </div>
    }
}
