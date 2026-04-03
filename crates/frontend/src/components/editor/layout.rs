use common::instrument::Layout;
use leptos::prelude::*;
use tauri_use::{use_command, use_invoke, UseTauriReturn, UseTauriWithReturn};

use crate::components::{Button, RangeSlider, SliderValue};

const SPACE_MIN: f32 = 64.0;
const SPACE_MAX: f32 = 7680.0;
const SPACE_STEP: f32 = 1.0;

const DIM_MIN: f32 = 0.0;
const DIM_MAX: f32 = 4000.0;
const DIM_STEP: f32 = 0.1;

const SAFE_MIN: f32 = 0.0;
const SAFE_MAX: f32 = 1000.0;
const SAFE_STEP: f32 = 0.5;

const U8_MIN: f32 = 1.0;
const U8_MAX: f32 = 32.0;
const U8_STEP: f32 = 1.0;

pub struct UseLayoutEditorReturn {
    pub layout: Signal<Layout>,
    pub set_layout: WriteSignal<Layout>,
    pub resize_locked: Signal<bool>,
    pub refresh: Callback<()>,
    pub submit: Callback<()>,
    pub set_resize_lock: Callback<bool>,
}

pub fn use_layout_editor() -> UseLayoutEditorReturn {
    let (layout, set_layout) = signal(Layout::default());
    let (resize_locked, set_resize_locked_signal) = signal(false);

    let UseTauriWithReturn {
        data: fetch_layout_data,
        trigger: fetch_layout,
        error: fetch_layout_error,
        ..
    } = use_command::<Layout>(common::commands::instrument::GET_LAYOUT);

    let UseTauriWithReturn {
        data: fetch_lock_data,
        trigger: fetch_lock,
        error: fetch_lock_error,
        ..
    } = use_command::<bool>(common::commands::instrument::GET_LAYOUT_RESIZE_LOCK);

    let UseTauriReturn {
        trigger: apply_layout,
        data: apply_layout_data,
        error: apply_layout_error,
        ..
    } = use_invoke::<Layout, (), Layout>(common::commands::instrument::SET_LAYOUT);

    let UseTauriReturn {
        trigger: apply_lock,
        data: apply_lock_data,
        error: apply_lock_error,
        ..
    } = use_invoke::<common::commands::instrument::LayoutResizeLockPayload, (), bool>(
        common::commands::instrument::SET_LAYOUT_RESIZE_LOCK,
    );

    let fetch_layout_on_mount = fetch_layout;
    let fetch_lock_on_mount = fetch_lock;
    Effect::new(move |_| {
        fetch_layout_on_mount(Some(()));
        fetch_lock_on_mount(Some(()));
    });

    Effect::new(move |_| {
        if let Some(v) = fetch_layout_data() {
            set_layout.set(v);
        }
    });

    Effect::new(move |_| {
        if let Some(v) = apply_layout_data() {
            set_layout.set(v);
        }
    });

    Effect::new(move |_| {
        if let Some(v) = fetch_lock_data() {
            set_resize_locked_signal.set(v);
        }
    });

    Effect::new(move |_| {
        if let Some(v) = apply_lock_data() {
            set_resize_locked_signal.set(v);
        }
    });

    Effect::new(move |_| {
        if let Some(err) = fetch_layout_error.get() {
            log::error!("Error fetching layout: {err}");
        }
        if let Some(err) = apply_layout_error.get() {
            log::error!("Error applying layout: {err}");
        }
        if let Some(err) = fetch_lock_error.get() {
            log::error!("Error fetching layout resize lock: {err}");
        }
        if let Some(err) = apply_lock_error.get() {
            log::error!("Error setting layout resize lock: {err}");
        }
    });

    let refresh_layout = fetch_layout;
    let refresh_lock = fetch_lock;
    let refresh = Callback::new(move |_| {
        refresh_layout(Some(()));
        refresh_lock(Some(()));
    });

    let submit = Callback::new({
        let layout = layout;
        move |_| {
            apply_layout(Some((layout(), ())));
        }
    });

    let set_resize_lock = Callback::new(move |locked: bool| {
        apply_lock(Some((
            common::commands::instrument::LayoutResizeLockPayload { locked },
            (),
        )));
    });

    UseLayoutEditorReturn {
        layout: layout.into(),
        set_layout,
        resize_locked: resize_locked.into(),
        refresh,
        submit,
        set_resize_lock,
    }
}

fn slider_input<F>(set_layout: WriteSignal<Layout>, apply: F) -> Callback<SliderValue>
where
    F: Fn(&mut Layout, f64) + Send + Sync + 'static,
{
    Callback::new(move |val: SliderValue| {
        let value: f32 = val.into();
        set_layout.update(|layout| apply(layout, value as f64));
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

#[component]
pub fn LayoutEditorPanel() -> impl IntoView {
    let UseLayoutEditorReturn {
        layout,
        set_layout,
        resize_locked,
        refresh,
        submit,
        set_resize_lock,
    } = use_layout_editor();

    let on_submit = move |ev: web_sys::SubmitEvent| {
        ev.prevent_default();
        submit.run(());
    };

    let on_reset = move |_: web_sys::MouseEvent| {
        refresh.run(());
    };

    view! {
        <div class="w-full h-full min-h-0 overflow-auto px-4 py-4 md:px-8 md:py-6">
            <div class="mx-auto flex w-full max-w-4xl flex-col gap-6">
                <form on:submit=on_submit>
                    <div class="flex w-full flex-col gap-4">
                        <Fold title="Resize Behavior">
                            <label class="inline-flex items-center gap-3">
                                <input
                                    type="checkbox"
                                    prop:checked=move || resize_locked()
                                    on:change:target=move |ev| {
                                        set_resize_lock.run(ev.target().checked());
                                    }
                                />
                                <span class="text-sm">
                                    "Prevent layout updates on window resize"
                                </span>
                            </label>
                        </Fold>

                        <Fold open=false title="Space">
                            <fieldset class="flex flex-col gap-2">
                                <RangeSlider
                                    show_value=true
                                    label="Space X"
                                    value=Signal::derive(move || (layout().space.x as f32).into())
                                    min=SPACE_MIN
                                    max=SPACE_MAX
                                    step=SPACE_STEP
                                    on_input={
                                        let set_layout = set_layout;
                                        slider_input(set_layout, |l, v| l.space.x = v.max(1.0))
                                    }
                                />
                                <RangeSlider
                                    show_value=true
                                    label="Space Y"
                                    value=Signal::derive(move || (layout().space.y as f32).into())
                                    min=SPACE_MIN
                                    max=SPACE_MAX
                                    step=SPACE_STEP
                                    on_input={
                                        let set_layout = set_layout;
                                        slider_input(set_layout, |l, v| l.space.y = v.max(1.0))
                                    }
                                />
                            </fieldset>
                        </Fold>

                        <Fold open=false title="Geometry">
                            <fieldset class="flex flex-col gap-2">
                                <RangeSlider
                                    show_value=true
                                    label="Instrument Breadth"
                                    value=Signal::derive(move || {
                                        (layout().instrument_breadth as f32).into()
                                    })
                                    min=DIM_MIN
                                    max=DIM_MAX
                                    step=DIM_STEP
                                    on_input={
                                        let set_layout = set_layout;
                                        slider_input(
                                            set_layout,
                                            |l, v| l.instrument_breadth = v.max(0.0),
                                        )
                                    }
                                />
                                <RangeSlider
                                    show_value=true
                                    label="Key Radius"
                                    value=Signal::derive(move || {
                                        (layout().key_radius as f32).into()
                                    })
                                    min=DIM_MIN
                                    max=DIM_MAX
                                    step=DIM_STEP
                                    on_input={
                                        let set_layout = set_layout;
                                        slider_input(set_layout, |l, v| l.key_radius = v.max(0.0))
                                    }
                                />
                                <RangeSlider
                                    show_value=true
                                    label="Key Band Length"
                                    value=Signal::derive(move || {
                                        (layout().key_band_length as f32).into()
                                    })
                                    min=DIM_MIN
                                    max=DIM_MAX
                                    step=DIM_STEP
                                    on_input={
                                        let set_layout = set_layout;
                                        slider_input(
                                            set_layout,
                                            |l, v| l.key_band_length = v.max(0.0),
                                        )
                                    }
                                />
                                <RangeSlider
                                    show_value=true
                                    label="Key Band Breadth"
                                    value=Signal::derive(move || {
                                        (layout().key_band_breadth as f32).into()
                                    })
                                    min=DIM_MIN
                                    max=DIM_MAX
                                    step=DIM_STEP
                                    on_input={
                                        let set_layout = set_layout;
                                        slider_input(
                                            set_layout,
                                            |l, v| l.key_band_breadth = v.max(0.0),
                                        )
                                    }
                                />
                                <RangeSlider
                                    show_value=true
                                    label="Key Bands Gap"
                                    value=Signal::derive(move || {
                                        (layout().key_bands_gap as f32).into()
                                    })
                                    min=DIM_MIN
                                    max=DIM_MAX
                                    step=DIM_STEP
                                    on_input={
                                        let set_layout = set_layout;
                                        slider_input(
                                            set_layout,
                                            |l, v| l.key_bands_gap = v.max(0.0),
                                        )
                                    }
                                />
                                <RangeSlider
                                    show_value=true
                                    label="Groups Gap"
                                    value=Signal::derive(move || {
                                        (layout().groups_gap as f32).into()
                                    })
                                    min=DIM_MIN
                                    max=DIM_MAX
                                    step=DIM_STEP
                                    on_input={
                                        let set_layout = set_layout;
                                        slider_input(set_layout, |l, v| l.groups_gap = v.max(0.0))
                                    }
                                />
                            </fieldset>
                        </Fold>

                        <Fold open=false title="Safe Area">
                            <fieldset class="flex flex-col gap-2">
                                <RangeSlider
                                    show_value=true
                                    label="Top"
                                    value=Signal::derive(move || {
                                        (layout().safe_area_padding.top as f32).into()
                                    })
                                    min=SAFE_MIN
                                    max=SAFE_MAX
                                    step=SAFE_STEP
                                    on_input={
                                        let set_layout = set_layout;
                                        slider_input(
                                            set_layout,
                                            |l, v| l.safe_area_padding.top = v.max(0.0),
                                        )
                                    }
                                />
                                <RangeSlider
                                    show_value=true
                                    label="Right"
                                    value=Signal::derive(move || {
                                        (layout().safe_area_padding.right as f32).into()
                                    })
                                    min=SAFE_MIN
                                    max=SAFE_MAX
                                    step=SAFE_STEP
                                    on_input={
                                        let set_layout = set_layout;
                                        slider_input(
                                            set_layout,
                                            |l, v| l.safe_area_padding.right = v.max(0.0),
                                        )
                                    }
                                />
                                <RangeSlider
                                    show_value=true
                                    label="Bottom"
                                    value=Signal::derive(move || {
                                        (layout().safe_area_padding.bottom as f32).into()
                                    })
                                    min=SAFE_MIN
                                    max=SAFE_MAX
                                    step=SAFE_STEP
                                    on_input={
                                        let set_layout = set_layout;
                                        slider_input(
                                            set_layout,
                                            |l, v| l.safe_area_padding.bottom = v.max(0.0),
                                        )
                                    }
                                />
                                <RangeSlider
                                    show_value=true
                                    label="Left"
                                    value=Signal::derive(move || {
                                        (layout().safe_area_padding.left as f32).into()
                                    })
                                    min=SAFE_MIN
                                    max=SAFE_MAX
                                    step=SAFE_STEP
                                    on_input={
                                        let set_layout = set_layout;
                                        slider_input(
                                            set_layout,
                                            |l, v| l.safe_area_padding.left = v.max(0.0),
                                        )
                                    }
                                />
                            </fieldset>
                        </Fold>

                        <Fold open=false title="Groups">
                            <fieldset class="flex flex-col gap-2">
                                <RangeSlider
                                    show_value=true
                                    label="Keys / Group"
                                    value=Signal::derive(move || {
                                        (layout().num_keys_per_group.get() as f32).into()
                                    })
                                    min=U8_MIN
                                    max=U8_MAX
                                    step=U8_STEP
                                    on_input=Callback::new({
                                        let set_layout = set_layout;
                                        move |val: SliderValue| {
                                            let value: f32 = val.into();
                                            let raw = value.round().clamp(U8_MIN, U8_MAX) as u8;
                                            if let Some(nz) = std::num::NonZero::new(raw) {
                                                set_layout.update(|l| l.num_keys_per_group = nz);
                                            }
                                        }
                                    })
                                />
                                <RangeSlider
                                    show_value=true
                                    label="Groups"
                                    value=Signal::derive(move || {
                                        (layout().num_groups.get() as f32).into()
                                    })
                                    min=U8_MIN
                                    max=U8_MAX
                                    step=U8_STEP
                                    on_input=Callback::new({
                                        let set_layout = set_layout;
                                        move |val: SliderValue| {
                                            let value: f32 = val.into();
                                            let raw = value.round().clamp(U8_MIN, U8_MAX) as u8;
                                            if let Some(nz) = std::num::NonZero::new(raw) {
                                                set_layout.update(|l| l.num_groups = nz);
                                            }
                                        }
                                    })
                                />
                            </fieldset>
                        </Fold>
                    </div>

                    <div class="mt-6 flex flex-wrap items-center gap-3">
                        <Button attr:r#type="submit">"Apply"</Button>
                        <Button attr:r#type="button" on:click=on_reset>
                            "Reset"
                        </Button>
                    </div>
                </form>
            </div>
        </div>
    }
}
