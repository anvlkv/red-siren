use super::UiSize;
use common::orientation::LayoutOrientation;
use leptos::{ev, html, prelude::*};
use leptos_use::{
    use_document, use_element_bounding_with_options, use_event_listener, UseElementBoundingOptions,
    UseElementBoundingReturn,
};

/// A slider value can be single or a range (start, end)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SliderValue {
    Single(f32),
    Range(f32, f32),
}

impl SliderValue {
    pub fn to_f32(self) -> f32 {
        match self {
            SliderValue::Single(v) => v,
            SliderValue::Range(v1, _) => v1,
        }
    }

    pub fn to_tuple(self) -> (f32, f32) {
        match self {
            SliderValue::Single(v) => (v, v),
            SliderValue::Range(v1, v2) => (v1, v2),
        }
    }
}

impl From<f32> for SliderValue {
    fn from(value: f32) -> Self {
        SliderValue::Single(value)
    }
}

impl From<(f32, f32)> for SliderValue {
    fn from(value: (f32, f32)) -> Self {
        SliderValue::Range(value.0, value.1)
    }
}

#[allow(clippy::from_over_into)]
impl Into<f32> for SliderValue {
    fn into(self) -> f32 {
        self.to_f32()
    }
}

#[allow(clippy::from_over_into)]
impl Into<(f32, f32)> for SliderValue {
    fn into(self) -> (f32, f32) {
        self.to_tuple()
    }
}

/// Internal helpers
fn snap_to_step(v: f32, min: f32, step: f32) -> f32 {
    if step <= 0.0 {
        return v;
    }
    let rel = v - min;
    let steps = (rel / step).round();
    min + steps * step
}

fn pct(v: f32, min: f32, max: f32) -> f32 {
    if (max - min).abs() < f32::EPSILON {
        0.0
    } else {
        ((v - min) / (max - min)).clamp(0.0, 1.0) * 100.0
    }
}

/// Format a value with precision derived from the step size.
fn format_value(v: f32, step: f32) -> String {
    let decimals = if step >= 1.0 {
        0usize
    } else {
        let s = format!("{}", step);
        s.find('.').map(|pos| s.len() - pos - 1).unwrap_or(2)
    };
    format!("{:.prec$}", v, prec = decimals)
}

/// RangeSlider component:
/// - Supports horizontal and vertical orientation
/// - Supports SliderValue::Single and SliderValue::Range
/// - Visually shows active selection on the track
/// - Validates min/max (panics if invalid)
#[component]
pub fn RangeSlider(
    #[prop(optional, into)] label: Signal<String>,
    #[prop(optional, into)] class: Signal<String>,
    #[prop(into)] value: Signal<SliderValue>,
    #[prop(optional, into)] min: Signal<f32>,
    #[prop(optional, into)] max: Signal<f32>,
    #[prop(optional, into)] step: Signal<f32>,
    #[prop(into)] on_input: Callback<SliderValue>,
    #[prop(optional, into)] orientation: Signal<LayoutOrientation>,
    #[prop(optional)] ui_size: UiSize,
    #[prop(optional, into)] min_label: Signal<String>,
    #[prop(optional, into)] max_label: Signal<String>,
    #[prop(optional, into)] show_value: Signal<bool>,
    #[prop(optional)] node_ref: NodeRef<html::Label>,
) -> impl IntoView {
    // Validate min/max (fail-fast)
    Effect::new(move |_| {
        let min_v = min();
        let max_v = max();
        if min_v >= max_v {
            panic!("RangeSlider: min must be strictly less than max. Got min={min_v}, max={max_v}");
        }
    });

    let min_v = min;
    let max_v = max;
    let step_v = Signal::derive(move || {
        let s = step();
        if s <= 0.0 {
            0.01
        } else {
            s
        }
    });

    // Normalize incoming value into ordered (lo, hi), snapped and clamped
    let lo_val = Signal::derive(move || match value() {
        SliderValue::Single(v) => snap_to_step(v, min_v(), step_v()).clamp(min_v(), max_v()),
        SliderValue::Range(v1, v2) => {
            let mut lo = snap_to_step(v1, min_v(), step_v()).clamp(min_v(), max_v());
            let mut hi = snap_to_step(v2, min_v(), step_v()).clamp(min_v(), max_v());
            if lo > hi {
                std::mem::swap(&mut lo, &mut hi);
            }
            lo
        }
    });
    let hi_val = Signal::derive(move || match value() {
        SliderValue::Single(v) => snap_to_step(v, min_v(), step_v()).clamp(min_v(), max_v()),
        SliderValue::Range(v1, v2) => {
            let mut lo = snap_to_step(v1, min_v(), step_v()).clamp(min_v(), max_v());
            let mut hi = snap_to_step(v2, min_v(), step_v()).clamp(min_v(), max_v());
            if lo > hi {
                std::mem::swap(&mut lo, &mut hi);
            }
            hi
        }
    });

    // Active segment overlay style
    let track_style = Signal::derive(move || {
        let a = lo_val();
        let b = hi_val();
        let p1 = pct(a, min_v(), max_v());
        let p2 = pct(b, min_v(), max_v());
        match orientation() {
            LayoutOrientation::Horizontal => format!(
                "position:absolute;left:{:.4}%;width:{:.4}%;",
                p1,
                (p2 - p1).abs()
            ),
            LayoutOrientation::Vertical => format!(
                "position:absolute;bottom:{:.4}%;height:{:.4}%;",
                p1,
                (p2 - p1).abs()
            ),
        }
    });

    // Track bounding for pointer math
    let track_ref = NodeRef::new();
    let UseElementBoundingReturn {
        height: track_h,
        width: track_w,
        left: track_left,
        top: track_top,
        update: update_track_bbox,
        ..
    } = use_element_bounding_with_options(
        track_ref,
        UseElementBoundingOptions {
            reset: true,
            window_resize: true,
            window_scroll: false,
            immediate: false,
        },
    );
    Effect::new(move |_| update_track_bbox());

    // Drag state: (which_thumb, pointer_id)
    // which_thumb: 0 = single/low, 1 = high
    let (drag_state, set_drag_state) = signal(Option::<(u8, i32)>::None);

    // Map pointer position to value in [min,max]
    let pointer_to_value = move |x: i32, y: i32| -> f32 {
        let minf = min_v();
        let maxf = max_v();
        let span = (maxf - minf).max(f32::EPSILON);
        let (w, h, l, t) = (track_w(), track_h(), track_left(), track_top());
        if orientation() == LayoutOrientation::Horizontal {
            let rel = (((x as f64 - l) / w.max(1.0)).clamp(0.0, 1.0)) as f32;
            let v = minf + rel * span;
            snap_to_step(v, minf, step_v()).clamp(minf, maxf)
        } else {
            // Vertical goes bottom-up
            let rel = (1.0 - ((y as f64 - t) / h.max(1.0)).clamp(0.0, 1.0)) as f32;
            let v = minf + rel * span;
            snap_to_step(v, minf, step_v()).clamp(minf, maxf)
        }
    };

    // Document-level pointer listeners (drag)
    let _unlisten_move = use_event_listener(use_document(), ev::pointermove, {
        move |ev| {
            if let Some((which, pid)) = drag_state() {
                if ev.pointer_id() != pid {
                    return;
                }
                let v = pointer_to_value(ev.client_x(), ev.client_y());
                match value() {
                    SliderValue::Single(_) => {
                        on_input.run(SliderValue::Single(v));
                    }
                    SliderValue::Range(_, _) => {
                        let lo = lo_val();
                        let hi = hi_val();
                        if which == 0 {
                            let new_lo = v.min(hi);
                            on_input.run(SliderValue::Range(new_lo, hi));
                        } else {
                            let new_hi = v.max(lo);
                            on_input.run(SliderValue::Range(lo, new_hi));
                        }
                    }
                }
            }
        }
    });
    let _unlisten_up = use_event_listener(use_document(), ev::pointerup, move |ev| {
        if let Some((_, pid)) = drag_state() {
            if ev.pointer_id() == pid {
                set_drag_state.set(None);
            }
        }
    });

    // Keyboard handlers
    let key_adjust = move |which: u8, key: String| {
        let step = step_v();
        let big = step * 10.0;
        let minf = min_v();
        let maxf = max_v();
        let (mut lo, mut hi) = (lo_val(), hi_val());
        let delta = match key.as_str() {
            "ArrowLeft" | "ArrowDown" => -step,
            "ArrowRight" | "ArrowUp" => step,
            "PageDown" => -big,
            "PageUp" => big,
            _ => 0.0,
        };
        if key == "Home" {
            if matches!(value(), SliderValue::Single(_)) || which == 0 {
                lo = minf;
            } else {
                hi = minf;
            }
        } else if key == "End" {
            if matches!(value(), SliderValue::Single(_)) || which == 0 {
                lo = maxf;
            } else {
                hi = maxf;
            }
        } else if delta != 0.0 {
            if matches!(value(), SliderValue::Single(_)) {
                lo = snap_to_step(lo + delta, minf, step).clamp(minf, maxf);
                hi = lo;
            } else if which == 0 {
                lo = snap_to_step(lo + delta, minf, step)
                    .clamp(minf, maxf)
                    .min(hi);
            } else {
                hi = snap_to_step(hi + delta, minf, step)
                    .clamp(minf, maxf)
                    .max(lo);
            }
        } else {
            return;
        }
        if matches!(value(), SliderValue::Single(_)) {
            on_input.run(SliderValue::Single(lo));
        } else {
            on_input.run(SliderValue::Range(lo, hi));
        }
    };

    // Track click: jump to position and start dragging nearest thumb
    let on_track_pointerdown = {
        move |ev: ev::PointerEvent| {
            ev.prevent_default();
            let v = pointer_to_value(ev.client_x(), ev.client_y());
            match value() {
                SliderValue::Single(_) => {
                    on_input.run(SliderValue::Single(v));
                    set_drag_state.set(Some((0, ev.pointer_id())));
                }
                SliderValue::Range(_, _) => {
                    let lo = lo_val();
                    let hi = hi_val();
                    // choose nearest thumb
                    let which = if (v - lo).abs() <= (v - hi).abs() {
                        0
                    } else {
                        1
                    };
                    if which == 0 {
                        let new_lo = v.min(hi);
                        on_input.run(SliderValue::Range(new_lo, hi));
                    } else {
                        let new_hi = v.max(lo);
                        on_input.run(SliderValue::Range(lo, new_hi));
                    }
                    set_drag_state.set(Some((which, ev.pointer_id())));
                }
            }
        }
    };

    // UI
    view! {
        <label
            class=move || { format!("flex flex-col gap-1 p-3 select-none {}", class()) }
            node_ref=node_ref
        >
            {move || {
                let has_label = !label().is_empty();
                let show_v = show_value();
                if !has_label && !show_v {
                    return ().into_any();
                }
                let value_display = if show_v {
                    let is_range = matches!(value(), SliderValue::Range(_, _));
                    if is_range {
                        view! {
                            <span class="inline-flex items-baseline gap-1 text-xs font-mono tabular-nums">
                                <ValueInput
                                    value=lo_val
                                    min=min_v
                                    max=max_v
                                    step=step_v
                                    on_commit=Callback::new(move |v: f32| {
                                        on_input.run(SliderValue::Range(v, hi_val()));
                                    })
                                />
                                <span class="text-muted-foreground">"–"</span>
                                <ValueInput
                                    value=hi_val
                                    min=min_v
                                    max=max_v
                                    step=step_v
                                    on_commit=Callback::new(move |v: f32| {
                                        on_input.run(SliderValue::Range(lo_val(), v));
                                    })
                                />
                            </span>
                        }
                            .into_any()
                    } else {
                        view! {
                            <ValueInput
                                value=lo_val
                                min=min_v
                                max=max_v
                                step=step_v
                                on_commit=Callback::new(move |v: f32| {
                                    on_input.run(SliderValue::Single(v));
                                })
                            />
                        }
                            .into_any()
                    }
                } else {
                    ().into_any()
                };
                view! {
                    <div class="flex items-baseline justify-between gap-2">
                        <span class="text-xs text-muted-foreground">{label()}</span>
                        {value_display}
                    </div>
                }
                    .into_any()
            }}
            <div
                node_ref=track_ref
                on:pointerdown=on_track_pointerdown
                class=move || match orientation() {
                    LayoutOrientation::Horizontal => {
                        match ui_size {
                            UiSize::Sm => "relative h-5 py-1 w-full flex items-center",
                            UiSize::Md => "relative h-6 py-1.5 w-full flex items-center",
                            UiSize::Lg => "relative h-7 py-2 w-full flex items-center",
                        }
                    }
                    LayoutOrientation::Vertical => {
                        match ui_size {
                            UiSize::Sm => "relative w-5 px-1 h-full flex justify-center",
                            UiSize::Md => "relative w-6 px-1.5 h-full flex justify-center",
                            UiSize::Lg => "relative w-7 px-2 h-full flex justify-center",
                        }
                    }
                }
            >
                // Base track
                <div class=move || match orientation() {
                    LayoutOrientation::Horizontal => {
                        match ui_size {
                            UiSize::Sm => {
                                "absolute left-0 right-0 top-1/2 -translate-y-1/2 h-1 rounded bg-black/15 dark:bg-red/15"
                            }
                            UiSize::Md => {
                                "absolute left-0 right-0 top-1/2 -translate-y-1/2 h-1.5 rounded bg-black/15 dark:bg-red/15"
                            }
                            UiSize::Lg => {
                                "absolute left-0 right-0 top-1/2 -translate-y-1/2 h-2 rounded bg-black/15 dark:bg-red/15"
                            }
                        }
                    }
                    LayoutOrientation::Vertical => {
                        match ui_size {
                            UiSize::Sm => {
                                "absolute top-0 bottom-0 left-1/2 -translate-x-1/2 w-1 rounded bg-black/15 dark:bg-red/15"
                            }
                            UiSize::Md => {
                                "absolute top-0 bottom-0 left-1/2 -translate-x-1/2 w-1.5 rounded bg-black/15 dark:bg-red/15"
                            }
                            UiSize::Lg => {
                                "absolute top-0 bottom-0 left-1/2 -translate-x-1/2 w-2 rounded bg-black/15 dark:bg-red/15"
                            }
                        }
                    }
                }></div>

                // Active segment
                <div
                    class=move || {
                        format!(
                            "rounded-lg {} bg-gray dark:bg-cinnabar",
                            match orientation() {
                                LayoutOrientation::Vertical => {
                                    match ui_size {
                                        UiSize::Sm => "left-1/2 -translate-x-1/2 w-1",
                                        UiSize::Md => "left-1/2 -translate-x-1/2 w-1.5",
                                        UiSize::Lg => "left-1/2 -translate-x-1/2 w-2",
                                    }
                                }
                                LayoutOrientation::Horizontal => {
                                    match ui_size {
                                        UiSize::Sm => "top-1/2 -translate-y-1/2 h-1",
                                        UiSize::Md => "top-1/2 -translate-y-1/2 h-1.5",
                                        UiSize::Lg => "top-1/2 -translate-y-1/2 h-2",
                                    }
                                }
                            },
                        )
                    }
                    style=track_style
                ></div>

                // Axis labels
                {move || {
                    let min_label_text = min_label();
                    let max_label_text = max_label();
                    let has_min_label = !min_label_text.is_empty();
                    let has_max_label = !max_label_text.is_empty();

                    view! {
                        <span
                            class="absolute left-0 -bottom-3 text-[10px] text-muted-foreground"
                            class:hidden=move || {
                                orientation() != LayoutOrientation::Horizontal || !has_min_label
                            }
                        >
                            {min_label_text.clone()}
                        </span>
                        <span
                            class="absolute right-0 -bottom-3 text-[10px] text-muted-foreground"
                            class:hidden=move || {
                                orientation() != LayoutOrientation::Horizontal || !has_max_label
                            }
                        >
                            {max_label_text.clone()}
                        </span>
                        <span
                            class="absolute bottom-0 left-1/2 -translate-x-1/2 translate-y-1 text-[10px] text-muted-foreground"
                            class:hidden=move || {
                                orientation() != LayoutOrientation::Vertical || !has_min_label
                            }
                        >
                            {min_label_text.clone()}
                        </span>
                        <span
                            class="absolute top-0 left-1/2 -translate-x-1/2 -translate-y-1 text-[10px] text-muted-foreground"
                            class:hidden=move || {
                                orientation() != LayoutOrientation::Vertical || !has_max_label
                            }
                        >
                            {max_label_text}
                        </span>
                    }
                }}

                // Thumbs (custom, draggable, keyboard-accessible)
                {move || match value() {
                    SliderValue::Single(_) => {
                        let val = lo_val;
                        view! {
                            <Thumb
                                orientation=orientation
                                value=val
                                min=min_v
                                max=max_v
                                ui_size=ui_size
                                label=label()
                                aria_min=min_v
                                aria_max=max_v
                                on_pointerdown=Callback::new(move |ev: ev::PointerEvent| {
                                    ev.prevent_default();
                                    set_drag_state.set(Some((0, ev.pointer_id())));
                                })
                                on_keydown=Callback::new(move |ev: ev::KeyboardEvent| {
                                    key_adjust(0, ev.key());
                                    ev.prevent_default();
                                })
                            />
                        }
                            .into_any()
                    }
                    SliderValue::Range(_, _) => {
                        let lo = lo_val;
                        let hi = hi_val;
                        view! {
                            <Thumb
                                orientation=orientation
                                value=lo
                                min=min_v
                                max=max_v
                                ui_size=ui_size
                                label=min_label()
                                aria_min=min_v
                                aria_max=hi
                                on_pointerdown=Callback::new(move |ev: ev::PointerEvent| {
                                    ev.prevent_default();
                                    set_drag_state.set(Some((0, ev.pointer_id())));
                                })
                                on_keydown=Callback::new(move |ev: ev::KeyboardEvent| {
                                    key_adjust(0, ev.key());
                                    ev.prevent_default();
                                })
                            />
                            <Thumb
                                orientation=orientation
                                value=hi
                                min=min_v
                                max=max_v
                                ui_size=ui_size
                                label=max_label()
                                aria_min=lo
                                aria_max=max_v
                                on_pointerdown=Callback::new(move |ev: ev::PointerEvent| {
                                    ev.prevent_default();
                                    set_drag_state.set(Some((1, ev.pointer_id())));
                                })
                                on_keydown=Callback::new(move |ev: ev::KeyboardEvent| {
                                    key_adjust(1, ev.key());
                                    ev.prevent_default();
                                })
                            />
                        }
                            .into_any()
                    }
                }}
            </div>
        </label>
    }
}

/// A visual thumb rendered at the correct position over the track.
/// It mirrors the input's value but is purely visual. The input itself is invisible, layered beneath.
/// We keep Thumb simple and stateless: it derives its position solely from props.
#[component]
fn Thumb(
    #[prop(into)] orientation: Signal<LayoutOrientation>,
    #[prop(into)] value: Signal<f32>,
    #[prop(into)] min: Signal<f32>,
    #[prop(into)] max: Signal<f32>,
    #[prop(optional)] ui_size: UiSize,
    #[prop(optional, into)] label: String,
    #[prop(into)] aria_min: Signal<f32>,
    #[prop(into)] aria_max: Signal<f32>,
    #[prop(into)] on_pointerdown: Callback<ev::PointerEvent>,
    #[prop(into)] on_keydown: Callback<ev::KeyboardEvent>,
) -> impl IntoView {
    // Visual state (local)
    let (hovered, set_hovered) = signal(false);
    let (active, set_active) = signal(false);
    let (focused, set_focused) = signal(false);

    let style = Signal::derive(move || {
        let v = value();
        let p = pct(v, min(), max());
        match orientation() {
            LayoutOrientation::Horizontal => {
                format!(
                    "position:absolute;left:{:.4}%;top:50%;transform:translate(-50%,-50%);",
                    p
                )
            }
            LayoutOrientation::Vertical => {
                format!(
                    "position:absolute;bottom:{:.4}%;left:50%;transform:translate(-50%,50%);",
                    p
                )
            }
        }
    });
    let class = Signal::derive(move || {
        let (is_hovered, is_active, is_focused) = (hovered(), active(), focused());
        let (w, h) = match ui_size {
            UiSize::Sm => {
                if is_active || is_focused {
                    ("w-3.5", "h-3.5")
                } else if is_hovered {
                    ("w-3", "h-3")
                } else {
                    ("w-2.5", "h-2.5")
                }
            }
            UiSize::Md => {
                if is_active || is_focused {
                    ("w-4", "h-4")
                } else if is_hovered {
                    ("w-3.5", "h-3.5")
                } else {
                    ("w-3", "h-3")
                }
            }
            UiSize::Lg => {
                if is_active || is_focused {
                    ("w-5", "h-5")
                } else if is_hovered {
                    ("w-4", "h-4")
                } else {
                    ("w-3.5", "h-3.5")
                }
            }
        };
        let ring = if is_focused {
            " ring-2 ring-offset-1 ring-cinnabar"
        } else {
            ""
        };
        let shadow = if is_active {
            " shadow-lg"
        } else if is_hovered {
            " shadow-md"
        } else {
            " shadow"
        };
        format!(
            "z-10 {} {} rounded-full bg-black dark:bg-red{}{}",
            w, h, shadow, ring
        )
    });

    let aria_orientation = Signal::derive(move || match orientation() {
        LayoutOrientation::Horizontal => "horizontal",
        LayoutOrientation::Vertical => "vertical",
    });

    view! {
        <div
            style=style
            class=class
            role="slider"
            tabindex="0"
            aria-orientation=aria_orientation
            aria-label=move || label.clone()
            aria-valuemin=move || aria_min().to_string()
            aria-valuemax=move || aria_max().to_string()
            aria-valuenow=move || value().to_string()
            on:pointerdown=move |ev| {
                set_active.set(true);
                on_pointerdown.run(ev);
            }
            on:pointerup=move |_| set_active.set(false)
            on:mouseenter=move |_| set_hovered.set(true)
            on:mouseleave=move |_| {
                set_hovered.set(false);
                set_active.set(false);
            }
            on:keydown=move |ev| on_keydown.run(ev)
            on:focus=move |_| set_focused.set(true)
            on:blur=move |_| set_focused.set(false)
        ></div>
    }
}

#[component]
fn ValueInput(
    #[prop(into)] value: Signal<f32>,
    #[prop(into)] min: Signal<f32>,
    #[prop(into)] max: Signal<f32>,
    #[prop(into)] step: Signal<f32>,
    #[prop(into)] on_commit: Callback<f32>,
) -> impl IntoView {
    let (editing, set_editing) = signal(false);
    let (draft, set_draft) = signal(String::new());
    let input_ref: NodeRef<leptos::html::Input> = NodeRef::new();

    // Focus + select all text when entering edit mode
    Effect::new(move |_| {
        if editing() {
            if let Some(el) = input_ref.get() {
                let _ = el.focus();
                let _ = el.select();
            }
        }
    });

    let try_commit = move || {
        let text = draft.get_untracked();
        if let Ok(parsed) = text.trim().parse::<f32>() {
            let step_v = step.get_untracked();
            let min_v = min.get_untracked();
            let max_v = max.get_untracked();
            let snapped = snap_to_step(parsed, min_v, step_v).clamp(min_v, max_v);
            on_commit.run(snapped);
        }
        set_editing.set(false);
    };

    view! {
        <Show
            when=move || editing()
            fallback=move || {
                let v = value();
                let s = step();
                view! {
                    <span
                        class="cursor-pointer text-xs font-mono tabular-nums \
                         text-muted-foreground hover:text-black dark:hover:text-red \
                         transition-colors underline decoration-dotted"
                        title="Click to edit"
                        on:click=move |_| {
                            set_draft
                                .set(format_value(value.get_untracked(), step.get_untracked()));
                            set_editing.set(true);
                        }
                    >
                        {format_value(v, s)}
                    </span>
                }
            }
        >
            <input
                type="text"
                node_ref=input_ref
                class="w-20 px-1 text-xs font-mono tabular-nums bg-transparent \
                 border-b border-black/40 dark:border-red/40 \
                 focus:outline-none focus:border-black dark:focus:border-red"
                prop:value=move || draft()
                on:input:target=move |ev| set_draft.set(ev.target().value())
                on:keydown=move |ev: web_sys::KeyboardEvent| match ev.key().as_str() {
                    "Enter" => {
                        ev.prevent_default();
                        try_commit();
                    }
                    "Escape" => {
                        ev.prevent_default();
                        set_editing.set(false);
                    }
                    _ => {}
                }
                on:blur=move |_| try_commit()
            />
        </Show>
    }
}
