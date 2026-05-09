use common::commands::edit::FineTunedValuesPayload;
use leptos::prelude::*;
use tauri_use::{use_command, use_invoke, UseTauriReturn, UseTauriWithReturn};

use crate::components::{Button, RangeSlider, SliderValue};

const Q_MAX: f32 = 10.0;
const Q_MIN: f32 = 0.001;
const Q_STEP: f32 = 0.001;

pub struct UseFinetunedValuesReturn {
    pub values: Signal<FineTunedValuesPayload>,
    pub set_values: WriteSignal<FineTunedValuesPayload>,
    pub refresh: Callback<()>,
    pub submit: Callback<()>,
}

pub fn use_finetuned_values() -> UseFinetunedValuesReturn {
    let (finetuned_values, set_finetuned_values) = signal(FineTunedValuesPayload::default());

    let UseTauriWithReturn {
        data: fetch_values_data,
        trigger: fetch_values,
        error: fetch_values_error,
        ..
    } = use_command::<FineTunedValuesPayload>(common::commands::edit::GET_FINETUNED_VALUES);

    let UseTauriReturn {
        trigger: update_values,
        error: update_values_error,
        data: update_values_data,
        ..
    } = use_invoke::<FineTunedValuesPayload, (), FineTunedValuesPayload>(
        common::commands::edit::EDIT_FINETUNED_VALUES,
    );

    let fetch_values_on_mount = fetch_values;
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

    let refresh_trigger = fetch_values;
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

#[component]
fn Fold(
    title: &'static str,
    #[prop(optional, default = true)] open: bool,
    children: Children,
) -> impl IntoView {
    view! {
        <details
            class="rounded-xl border border-black/15 bg-black/5 p-4 dark:border-red/20 dark:bg-red/5"
            open=open
        >
            <summary class="cursor-pointer list-none text-sm font-semibold tracking-wide select-none">
                {title}
            </summary>
            <div class="mt-4 flex flex-col gap-4">{children()}</div>
        </details>
    }
}

/// Standalone editor surface for fine-tuning instrument parameters.
/// This is shared between the devtools page and any future inline surfaces.
#[component]
pub fn FinetunedValuesEditorPanel() -> impl IntoView {
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
        <div class="w-full h-full min-h-0 overflow-auto px-4 py-4 md:px-8 md:py-6">
            <div class="mx-auto flex w-full max-w-4xl flex-col gap-6">
                <form on:submit=on_submit>
                    <div class="flex w-full flex-col gap-4">
                        <Fold open=false title="Formant Parameters">
                            <fieldset class="flex flex-col gap-2">
                                <RangeSlider
                                    show_value=true
                                    label="Q"
                                    value=Signal::derive(move || values().formants_q.into())
                                    min=Q_MIN
                                    max=Q_MAX
                                    step=Q_STEP
                                    on_input={
                                        let set_values = set_values;
                                        slider_input(
                                            set_values,
                                            |payload, value| {
                                                payload.formants_q = value;
                                            },
                                        )
                                    }
                                />
                            </fieldset>
                        </Fold>

                        <div class="flex w-full flex-col gap-3 sm:flex-row sm:justify-stretch">
                            <Button attr:r#type="submit">"Apply Changes"</Button>
                            <Button attr:r#type="button" on:click=on_reset>
                                "Reset"
                            </Button>
                        </div>
                    </div>
                </form>
            </div>
        </div>
    }
}
