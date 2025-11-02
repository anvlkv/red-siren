use leptos::prelude::*;

#[component]
pub fn EditorRangeSlider(
    label: &'static str,
    value: Signal<f32>,
    min: f32,
    max: f32,
    step: f32,
    on_input: Callback<f32>,
) -> impl IntoView {
    view! {
        <label class="flex flex-col gap-1">
            <span class="block w-full">{label}</span>
            <input
                type="range"
                class="w-48"
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
            <span class="block w-full text-right">{move || format!("{:.3}", value())}</span>
        </label>
    }
}
