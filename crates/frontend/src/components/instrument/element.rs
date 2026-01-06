use common::{
    instrument::commands::{
        ReflectBandControlPayload, ReflectKeyControlPayload, UpdateBandControlPayload,
        UpdateKeyControlPayload,
    },
    NodeKey, NodeKeyRegistry,
};
use leptos::{ev, prelude::*};
use leptos_use::{
    use_document, use_element_bounding_with_options, use_event_listener, UseElementBoundingOptions,
    UseElementBoundingReturn,
};
use tauri_use::{use_invoke, use_listen, UseListenReturn, UseTauriReturn};
use web_sys::WheelEvent;
use web_time::Instant;

use crate::{
    components::{expect_instrument_context, instrument::context::Bounding, Button, UiSize},
    util::layout_context::{expect_layout_context, LayoutContextReturn},
};

#[component]
pub fn KeyboardElement(
    g: usize,
    k: usize,
    channel: common::instrument::GroupChannel,
    #[prop(into)] excitement_samples: Signal<Option<Vec<(f32, f32)>>>,
) -> impl IntoView {
    let ctx = expect_instrument_context();

    let LayoutContextReturn {
        key_radius,
        key_band_breadth,
        key_band_length,
        orientation,
        num_groups,
        num_keys_per_group,
        space,
        ..
    } = expect_layout_context();

    let (drag_start_pos, set_drag_start_pos) = signal(Option::<(i32, i32, i32)>::None);
    let (prev_instant, set_prev_instant) = signal(Instant::now());

    let registry = Signal::derive(move || NodeKeyRegistry::new(num_groups(), num_keys_per_group()));

    let UseTauriReturn {
        error: band_control_error,
        trigger: update_band_control,
        ..
    } = use_invoke::<UpdateBandControlPayload, (), ()>(
        common::commands::instrument::UPDATE_BAND_CONTROL,
    );

    let delta_change = move |d_x: i32, d_y: i32, alt: bool, ctrl: bool, instant: Instant| {
        let duration = instant.duration_since(prev_instant());
        set_prev_instant.set(instant);
        let delta = match orientation() {
            common::orientation::LayoutOrientation::Vertical => d_x,
            common::orientation::LayoutOrientation::Horizontal => d_y,
        } as f64
            * match channel {
                common::instrument::GroupChannel::Left => -1.0,
                common::instrument::GroupChannel::Right => 1.0,
            }
            * if ctrl { 0.1 } else { 1.0 };
        let bound = key_band_length();

        let velocity = if duration.as_millis() > 0 {
            delta / (duration.as_millis() as f64 / 1000.0)
        } else {
            0.0
        };
        // Scale movement fraction by velocity: small at low velocity, larger at high velocity.
        // v0 is a tuning constant that controls how quickly the scaling ramps up.
        let v0 = bound; // px/sec threshold
        let v_abs = velocity.abs();
        let scale = v_abs / (v_abs + v0);
        let frac = ((delta / bound) * scale).clamp(-1.0, 1.0) as f32;

        if ctrl {
            let keys = registry().all_keys();
            update_band_control.set(Some((
                UpdateBandControlPayload {
                    keys,
                    increment: frac,
                },
                (),
            )));
        } else if alt {
            let keys = registry().group_keys(g as u8);
            update_band_control.set(Some((
                UpdateBandControlPayload {
                    keys,
                    increment: frac,
                },
                (),
            )));
        } else {
            update_band_control.set(Some((
                UpdateBandControlPayload {
                    keys: vec![NodeKey(g as u8, k as u8)],
                    increment: frac,
                },
                (),
            )));
        }
    };

    let unlisten_pointermove = use_event_listener(use_document(), ev::pointermove, move |ev| {
        let pointer = ev.pointer_id();
        if let Some((start_x, start_y, _)) = drag_start_pos().filter(|(_, _, pid)| pointer == *pid)
        {
            let pos_x = ev.client_x();
            let d_x = start_x - pos_x;
            let pos_y = ev.client_y();
            let d_y = start_y - pos_y;
            let alt = ev.alt_key();
            let ctrl = ev.ctrl_key();
            delta_change(d_x, d_y, alt, ctrl, Instant::now());
            set_drag_start_pos.set(Some((pos_x, pos_y, pointer)));
        }
    });
    let unlisten_pointerup = use_event_listener(use_document(), ev::pointerup, move |ev| {
        let pointer = ev.pointer_id();
        if let Some((start_x, start_y, _)) = drag_start_pos().filter(|(_, _, pid)| pointer == *pid)
        {
            let pos_x = ev.client_x();
            let d_x = start_x - pos_x;
            let pos_y = ev.client_y();
            let d_y = start_y - pos_y;
            let alt = ev.alt_key();
            let ctrl = ev.ctrl_key();
            delta_change(d_x, d_y, alt, ctrl, Instant::now());
            set_drag_start_pos.set(None);
        }
    });

    let UseTauriReturn {
        error: key_control_error,
        trigger: update_key_control,
        ..
    } = use_invoke::<UpdateKeyControlPayload, (), ()>(
        common::commands::instrument::UPDATE_KEY_CONTROL,
    );

    let UseListenReturn {
        data: band_control_data,
        error: listen_band_control_error,
        open: listen_band_control_open,
        close: listen_band_control_close,
        ..
    } = use_listen::<ReflectBandControlPayload>(tauri_use::EventType::Custom(
        common::events::instrument::BAND_CONTROL_G_K,
    ));

    let UseListenReturn {
        data: key_control_data,
        error: listen_key_control_error,
        open: listen_key_control_open,
        close: listen_key_control_close,
        ..
    } = use_listen::<ReflectKeyControlPayload>(tauri_use::EventType::Custom(
        common::events::instrument::KEY_CONTROL_G_K,
    ));

    Effect::new(move |_| {
        listen_key_control_open();
        listen_band_control_open();
    });

    let band_control_pos = Memo::new(move |prev| {
        let radius = key_radius();
        let pad = key_band_breadth() - radius * 2.0;
        let len = key_band_length() - pad * 2.0;
        band_control_data()
            .iter()
            .find_map(|d| {
                if d.group == g as u8 && d.key == k as u8 {
                    let value = (len / 2.0) + (d.value as f64 * (len / 2.0));
                    Some(value - radius + pad / 2.0)
                } else {
                    None
                }
            })
            .or(prev.copied())
            .unwrap_or_else(|| len / 2.0 - radius + pad / 2.0)
    });

    let key_control_data = Memo::new(move |prev| {
        key_control_data()
            .iter()
            .find_map(|d| {
                if d.group == g as u8 && d.key == k as u8 {
                    Some(d.value > 0.0)
                } else {
                    None
                }
            })
            .or(prev.copied())
            .unwrap_or(false)
    });

    // Log errors from band control updates
    Effect::new(move |_| {
        if let Some(err) = band_control_error() {
            log::error!("Error calling update band control: {err}");
        }
        if let Some(err) = listen_band_control_error() {
            log::error!("Error listening to band control: {err}");
        }
        if let Some(err) = key_control_error() {
            log::error!("Error calling update key control: {err}");
        }
        if let Some(err) = listen_key_control_error() {
            log::error!("Error listening to key control: {err}");
        }
    });

    on_cleanup(move || {
        listen_band_control_close();
        listen_key_control_close();
        unlisten_pointermove();
        unlisten_pointerup();
    });

    let on_wheel = move |ev: WheelEvent| {
        ev.prevent_default();
        let _delta_mode = ev.delta_mode();
        let delta = ev.delta_y();
        let alt = ev.alt_key();
        let ctrl = ev.ctrl_key();
        let (d_x, d_y) = match orientation() {
            common::orientation::LayoutOrientation::Horizontal => (0, delta as i32),
            common::orientation::LayoutOrientation::Vertical => (delta as i32, 0),
        };
        delta_change(d_x, d_y, alt, ctrl, Instant::now());
    };

    let on_pointerup = move |ev: ev::PointerEvent| {
        let alt = ev.alt_key();
        let ctrl = ev.ctrl_key();
        let payload = if ctrl {
            let keys = registry().all_keys();
            UpdateKeyControlPayload {
                keys,
                value: if key_control_data() { 0.0 } else { 1.0 },
            }
        } else if alt {
            let keys = registry().group_keys(g as u8);
            UpdateKeyControlPayload {
                keys,
                value: if key_control_data() { 0.0 } else { 1.0 },
            }
        } else {
            UpdateKeyControlPayload {
                keys: vec![NodeKey(g as u8, k as u8)],
                value: if key_control_data() { 0.0 } else { 1.0 },
            }
        };
        update_key_control(Some((payload, ())));
        set_drag_start_pos.set(None);
    };

    let on_pointerdown = move |ev: ev::PointerEvent| {
        let pos_x = ev.client_x();
        let pos_y = ev.client_y();
        let id = ev.pointer_id();
        set_drag_start_pos.set(Some((pos_x, pos_y, id)));
    };

    let button_ref = NodeRef::new();

    let UseElementBoundingReturn {
        height: button_height,
        width: button_width,
        left: button_left,
        right: button_right,
        top: button_top,
        bottom: button_bottom,
        x: button_x,
        y: button_y,
        update,
    } = use_element_bounding_with_options(
        button_ref,
        UseElementBoundingOptions {
            reset: true,
            window_resize: true,
            window_scroll: false,
            immediate: false,
        },
    );

    Effect::new(move |_| update());

    let band_ref = NodeRef::new();

    let UseElementBoundingReturn {
        height: band_height,
        width: band_width,
        left: band_left,
        right: band_right,
        top: band_top,
        bottom: band_bottom,
        x: band_x,
        y: band_y,
        update,
    } = use_element_bounding_with_options(
        band_ref,
        UseElementBoundingOptions {
            reset: true,
            window_resize: true,
            window_scroll: false,
            immediate: false,
        },
    );

    Effect::new(move |_| update());

    let should_animate = RwSignal::new(false);

    Effect::new(move |_| {
        should_animate.set(
            ctx.upsert_key_bbox(
                (g, k),
                Bounding {
                    x: button_x(),
                    y: button_y(),
                    width: button_width(),
                    height: button_height(),
                    top: button_top(),
                    right: button_right(),
                    bottom: button_bottom(),
                    left: button_left(),
                },
            ) && ctx.upsert_band_bbox(
                (g, k),
                Bounding {
                    x: band_x(),
                    y: band_y(),
                    width: band_width(),
                    height: band_height(),
                    top: band_top(),
                    right: band_right(),
                    bottom: band_bottom(),
                    left: band_left(),
                },
            ),
        )
    });

    // Stage variables: collapse under sun (k1), then breadth-first and length unstack
    let key_stage1_vars = move || {
        let sun_r = crate::components::intro::consts::INTRO_SUN_RADIUS;
        let sun_x = crate::components::intro::consts::INTRO_SUN_POS_X;
        let sun_y = crate::components::intro::consts::INTRO_SUN_POS_Y;
        let scale = sun_r / key_radius();

        // Map sun position from artwork space to screen space
        let viewport_w = crate::components::intro::consts::INTRO_COMP_VIEWBOX_WIDTH;
        let viewport_h = crate::components::intro::consts::INTRO_COMP_VIEWBOX_HEIGHT;
        let layout_space = space();
        let screen_w = layout_space.x;
        let screen_h = layout_space.y;

        // Scale factors for artwork to screen
        let scale_x = screen_w / viewport_w;
        let scale_y = screen_h / viewport_h;

        // Use the smaller scale to maintain aspect ratio (fit within screen)
        let viewport_scale = scale_x.min(scale_y);

        // Calculate sun position in screen space
        let sun_screen_x = sun_x * viewport_scale;
        let sun_screen_y = sun_y * viewport_scale;

        // Align with sun position in screen space (center-to-center)
        let k1_tx = sun_screen_x - (button_x() + button_width() / 2.0);
        let k1_ty = sun_screen_y - (button_y() + button_height() / 2.0);

        // Breadth-first move: fix the axis orthogonal to main
        let (breadth_tx, breadth_ty) = match orientation() {
            common::orientation::LayoutOrientation::Vertical => (0.0, k1_ty),
            common::orientation::LayoutOrientation::Horizontal => (k1_tx, 0.0),
        };

        // Length (main) axis then unstack to final
        let (length_tx, length_ty) = (0.0, 0.0);

        format!(
            concat!(
                "--inst-key-k1-tx: {k1_tx}px;",
                " --inst-key-k1-ty: {k1_ty}px;",
                " --inst-key-k1-scale: {scale};",
                " --inst-key-k2-scale: 1;",
                " --inst-key-breadth-tx: {breadth_tx}px;",
                " --inst-key-breadth-ty: {breadth_ty}px;",
                " --inst-key-length-tx: {length_tx}px;",
                " --inst-key-length-ty: {length_ty}px;",
            ),
            k1_tx = k1_tx,
            k1_ty = k1_ty,
            scale = scale,
            breadth_tx = breadth_tx,
            breadth_ty = breadth_ty,
            length_tx = length_tx,
            length_ty = length_ty,
        )
    };

    view! {
        <div
            class=move || {
                format!(
                    "overflow-visible relative {}",
                    if should_animate() {
                        super::instrument_animations::INSTRUMENT_KEY_APPEAR
                    } else {
                        ""
                    },
                )
            }
            style=move || {
                format!(
                    "width: {size}px; height: {size}px; {vars}",
                    size = key_band_breadth(),
                    vars = key_stage1_vars(),
                )
            }
            role="slider"
            aria-orientation=move || {
                match orientation() {
                    common::orientation::LayoutOrientation::Vertical => "horizontal",
                    common::orientation::LayoutOrientation::Horizontal => "vertical",
                }
            }
            aria-valuemin="-1.0"
            aria-valuemax="1.0"
            aria-valuenow=move || {
                band_control_data()
                    .iter()
                    .find_map(|d| {
                        if d.group == g as u8 && d.key == k as u8 {
                            Some(format!("{}", d.value))
                        } else {
                            None
                        }
                    })
                    .unwrap_or_default()
            }
        >
            <div
                style:width=move || {
                    format!(
                        "{}px",
                        match orientation() {
                            common::orientation::LayoutOrientation::Vertical => key_band_length(),
                            common::orientation::LayoutOrientation::Horizontal => key_band_breadth(),
                        },
                    )
                }
                style:height=move || {
                    format!(
                        "{}px",
                        match orientation() {
                            common::orientation::LayoutOrientation::Vertical => key_band_breadth(),
                            common::orientation::LayoutOrientation::Horizontal => key_band_length(),
                        },
                    )
                }
                class=move || {
                    format!(
                        "absolute rounded-full bg-red/50 dark:bg-black/50 border-black dark:border-red backdrop-blur-md shadow-sm overflow-visible {} {}",
                        match (orientation(), channel) {
                            (
                                common::orientation::LayoutOrientation::Horizontal,
                                common::instrument::GroupChannel::Left,
                            ) => "top-0 border-t-1 border-x-3 border-b-5",
                            (
                                common::orientation::LayoutOrientation::Vertical,
                                common::instrument::GroupChannel::Left,
                            ) => "left-0 border-l-1 border-y-3 border-r-5",
                            (
                                common::orientation::LayoutOrientation::Horizontal,
                                common::instrument::GroupChannel::Right,
                            ) => "bottom-0 border-b-1 border-x-3 border-t-5",
                            (
                                common::orientation::LayoutOrientation::Vertical,
                                common::instrument::GroupChannel::Right,
                            ) => "right-0 border-r-1 border-y-3 border-l-5",
                        },
                        if should_animate() {
                            super::instrument_animations::INSTRUMENT_BAND_APPEAR
                        } else {
                            ""
                        },
                    )
                }
                on:wheel=on_wheel
                role="presentation"
                node_ref=band_ref
            ></div>
            <Button
                size=UiSize::Sm
                round=true
                square=true
                class=Signal::derive(move || {
                    format!(
                        "absolute transition-opacity {}",
                        if key_control_data() {
                            "inset-ring-3 inset-ring-cinnabar dark:inset-ring-gray ring-2 ring-gray dark:ring-cinnabar"
                        } else {
                            ""
                        },
                    )
                })
                style:top=move || {
                    if matches!(orientation(), common::orientation::LayoutOrientation::Horizontal) {
                        format!(
                            "{}px",
                            match channel {
                                common::instrument::GroupChannel::Left => band_control_pos(),
                                common::instrument::GroupChannel::Right => -band_control_pos(),
                            },
                        )
                    } else {
                        "".to_string()
                    }
                }
                style:left=move || {
                    if matches!(orientation(), common::orientation::LayoutOrientation::Vertical) {
                        format!(
                            "{}px",
                            match channel {
                                common::instrument::GroupChannel::Left => band_control_pos(),
                                common::instrument::GroupChannel::Right => -band_control_pos(),
                            },
                        )
                    } else {
                        "".to_string()
                    }
                }
                style:width=move || {
                    let dia = key_radius() * 2.0;
                    format!("{dia}px")
                }
                style:height=move || {
                    let dia = key_radius() * 2.0;
                    format!("{dia}px")
                }
                style:margin=move || {
                    let dia = key_radius() * 2.0;
                    let br = key_band_breadth();
                    let pad = ((br - dia) / 2.0).max(0.0);
                    format!("{pad}px")
                }
                style:transform=move || {
                    let samples = excitement_samples.get().unwrap_or_default();
                    let inc = samples.iter().map(|(v, _)| v.abs()).sum::<f32>();
                    format!("scale({s}, {s})", s = 0.75 + (inc / samples.len() as f32) * 0.275)
                }
                style:opacity=move || {
                    if excitement_samples
                        .get()
                        .unwrap_or_default()
                        .iter()
                        .all(|s| s.1 == 0.0 && s.0 == 0.0)
                    {
                        0.85
                    } else {
                        1.0
                    }
                        .to_string()
                }
                on:wheel=on_wheel
                on:pointerup=on_pointerup
                on:pointerdown=on_pointerdown
                node_ref=button_ref
            >
                <div
                    class="w-full h-full rounded-full bg-gray dark:bg-cinnabar mix-blend-plus-lighter"
                    style:opacity=move || {
                        let samples = excitement_samples.get().unwrap_or_default();
                        let inc = samples.iter().map(|(_, v)| v.abs()).sum::<f32>();
                        format!("{}", (inc / samples.len() as f32))
                    }
                ></div>
            </Button>
        </div>
    }
}
