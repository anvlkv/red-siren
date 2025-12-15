use common::instrument::commands::{UpdateBandControlPayload, UpdateKeyControlPayload};
use leptos::{html, prelude::*};
use leptos_use::{
    core::Position, use_draggable_with_options, use_element_bounding_with_options,
    UseDraggableOptions, UseDraggableReturn, UseElementBoundingOptions,
};
use tauri_use::{use_invoke, use_listen, UseListenReturn, UseTauriReturn};

use crate::{
    components::{expect_instrument_context, Button, UiSize},
    util::layout_context::{expect_layout_contex, LayoutContextReturn},
};

#[component]
pub fn KeyboardElement(
    g: usize,
    k: usize,
    first_group_channel: common::instrument::GroupChannel,
    orientation: common::orientation::LayoutOrientation,
    #[prop(into)] excitement_samples: Signal<Option<Vec<f32>>>,
) -> impl IntoView {
    let ctx = expect_instrument_context();
    let LayoutContextReturn {
        key_radius,
        space,
        num_keys_per_group,
        key_band_breadth,
        complete_layout,
        ..
    } = expect_layout_contex();

    // NodeRefs for band and key wrapper
    let band_ref = NodeRef::<html::Div>::new();
    let key_ref = NodeRef::<html::Div>::new();
    let draggable_ref = NodeRef::<html::Button>::new();

    // Measure band
    let leptos_use::UseElementBoundingReturn {
        x: band_x,
        y: band_y,
        top: band_top,
        right: band_right,
        bottom: band_bottom,
        left: band_left,
        width: band_width,
        height: band_height,
        update: band_update,
        ..
    } = use_element_bounding_with_options(
        band_ref,
        UseElementBoundingOptions::default().immediate(false),
    );

    // Measure key (wrapper)
    let leptos_use::UseElementBoundingReturn {
        x: key_x,
        y: key_y,
        top: key_top,
        right: key_right,
        bottom: key_bottom,
        left: key_left,
        width: key_width,
        height: key_height,
        update: key_update,
        ..
    } = use_element_bounding_with_options(
        key_ref,
        UseElementBoundingOptions::default().immediate(false),
    );

    // One-shot guards to attach animation classes only once per element
    let band_should_animate = RwSignal::new(false);
    let key_should_animate = RwSignal::new(false);

    // Upsert band bbox; animate on first measurement
    Effect::new({
        let ctx = ctx.clone();
        move |_| {
            let w = band_width();
            let h = band_height();
            if w > 0.0 && h > 0.0 {
                let bbox = super::context::Bounding {
                    x: band_x() as f32,
                    y: band_y() as f32,
                    width: w as f32,
                    height: h as f32,
                    top: band_top() as f32,
                    right: band_right() as f32,
                    bottom: band_bottom() as f32,
                    left: band_left() as f32,
                };
                if ctx.upsert_band_bbox((g, k), bbox) {
                    band_should_animate.set(true);
                }
            }
        }
    });

    // Upsert key bbox; animate on first measurement
    Effect::new({
        let ctx = ctx.clone();
        move |_| {
            let w = key_width();
            let h = key_height();
            if w > 0.0 && h > 0.0 {
                let bbox = super::context::Bounding {
                    x: key_x() as f32,
                    y: key_y() as f32,
                    width: w as f32,
                    height: h as f32,
                    top: key_top() as f32,
                    right: key_right() as f32,
                    bottom: key_bottom() as f32,
                    left: key_left() as f32,
                };
                if ctx.upsert_key_bbox((g, k), bbox) {
                    key_should_animate.set(true);
                }
            }
        }
    });

    // Backend update command
    let UseTauriReturn {
        error: band_control_error,
        trigger: update_band_control,
        ..
    } = use_invoke::<UpdateBandControlPayload, (), ()>(
        common::commands::instrument::UPDATE_BAND_CONTROL,
    );

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
    } = use_listen::<UpdateBandControlPayload>(tauri_use::EventType::Custom(
        common::events::instrument::BAND_CONTROL_G_K,
    ));

    let UseListenReturn {
        data: key_control_data,
        error: listen_key_control_error,
        open: listen_key_control_open,
        close: listen_key_control_close,
        ..
    } = use_listen::<UpdateKeyControlPayload>(tauri_use::EventType::Custom(
        common::events::instrument::KEY_CONTROL_G_K,
    ));

    // Local state for pressed/released
    let is_pressed = RwSignal::new(false);

    let band_control_data = Memo::new(move |prev| {
        band_control_data()
            .iter()
            .filter_map(|d| {
                if d.group == g as u8 && d.key == k as u8 {
                    Some(d.value)
                } else {
                    None
                }
            })
            .next()
            .or(prev.copied())
            .unwrap_or_default()
    });

    let key_control_data = Memo::new(move |prev| {
        key_control_data()
            .iter()
            .filter_map(|d| {
                if d.group == g as u8 && d.key == k as u8 {
                    Some(d.value)
                } else {
                    None
                }
            })
            .next()
            .or(prev.copied())
            .unwrap_or(0.0)
    });

    // Sync key control state
    Effect::new(move |_| {
        let value = key_control_data();
        is_pressed.set(value > 0.5);
    });

    // Get initial band control
    Effect::new(move |_| {
        listen_band_control_open();
    });

    // Get initial key control
    Effect::new(move |_| {
        listen_key_control_open();
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
    });

    // Calculate drag constraints based on band dimensions and alignment
    let calculated_drag_constraints = Memo::new(move |_| {
        let band_w = band_width.get();
        let band_h = band_height.get();
        let key_dia = key_radius.get() * 2.0;
        let key_margin = ((key_band_breadth.get() - key_dia) / 2.0).max(0.0);
        let key_full_size = key_dia + key_margin * 2.0;

        if band_w <= 0.0 || band_h <= 0.0 || key_full_size <= 0.0 {
            return (0.0, 0.0, 0.0, 0.0);
        }

        let channel = first_group_channel.nth_channel_from_first(g);

        match orientation {
            common::orientation::LayoutOrientation::Vertical => {
                // Vertical: keys stack vertically, drag horizontally along band's length (width)
                let available_x = (band_w - key_full_size).max(0.0);
                match channel {
                    common::instrument::GroupChannel::Left => {
                        // Left aligned: can drag from 0 to available_x
                        (0.0, available_x, 0.0, 0.0)
                    }
                    common::instrument::GroupChannel::Right => {
                        // Right aligned: can drag from -available_x to 0
                        (-available_x, 0.0, 0.0, 0.0)
                    }
                }
            }
            common::orientation::LayoutOrientation::Horizontal => {
                // Horizontal: keys stack horizontally, drag vertically along band's length (height)
                let available_y = (band_h - key_full_size).max(0.0);
                match channel {
                    common::instrument::GroupChannel::Left => {
                        // Top aligned: can drag from 0 to available_y
                        (0.0, 0.0, 0.0, available_y)
                    }
                    common::instrument::GroupChannel::Right => {
                        // Bottom aligned: can drag from -available_y to 0
                        (0.0, 0.0, -available_y, 0.0)
                    }
                }
            }
        }
    });

    // Setup draggable for the wrapper div
    let UseDraggableReturn {
        position: drag_position,
        is_dragging,
        ..
    } = use_draggable_with_options(
        draggable_ref,
        UseDraggableOptions::default().prevent_default(true),
    );

    // Constrain position and update backend
    let constrained_position = Signal::derive(move || {
        let pos = band_control_data.get() as f64;
        let (min_x, max_x, min_y, max_y) = calculated_drag_constraints();

        match (orientation, first_group_channel.nth_channel_from_first(g)) {
            (
                common::orientation::LayoutOrientation::Vertical,
                common::instrument::GroupChannel::Left,
            ) => {
                let x_range = max_x - min_x;
                let x = pos * x_range;
                Position { x, y: 0.0 }
            }
            (
                common::orientation::LayoutOrientation::Vertical,
                common::instrument::GroupChannel::Right,
            ) => {
                let x_range = max_x - min_x;
                let x = pos * -x_range;
                Position { x, y: 0.0 }
            }
            (
                common::orientation::LayoutOrientation::Horizontal,
                common::instrument::GroupChannel::Left,
            ) => {
                let y_range = max_y - min_y;
                let y = pos * y_range;
                Position { x: 0.0, y }
            }
            (
                common::orientation::LayoutOrientation::Horizontal,
                common::instrument::GroupChannel::Right,
            ) => {
                let y_range = max_y - min_y;
                let y = pos * -y_range;
                Position { x: 0.0, y }
            }
        }
    });

    // Calculate normalized value for band control
    Effect::new(move |_| {
        let Position { x, y } = drag_position.get();
        let (min_x, max_x, min_y, max_y) = calculated_drag_constraints();
        let base_x = key_x();
        let base_y = key_y();

        if is_dragging.get() {
            let normalized_value = match orientation {
                common::orientation::LayoutOrientation::Vertical => {
                    let x_range = max_x - min_x;

                    let d_x = match first_group_channel.nth_channel_from_first(g) {
                        common::instrument::GroupChannel::Left => x - base_x,
                        common::instrument::GroupChannel::Right => base_x - x,
                    };

                    log::debug!("d_x: {d_x}");

                    d_x / x_range
                }
                common::orientation::LayoutOrientation::Horizontal => {
                    let y_range = max_y - min_y;

                    let d_y = match first_group_channel.nth_channel_from_first(g) {
                        common::instrument::GroupChannel::Left => y - base_y,
                        common::instrument::GroupChannel::Right => base_y - y,
                    };

                    log::debug!("d_y: {d_y}");

                    d_y / y_range
                }
            }
            .clamp(0.0, 1.0);

            // Update backend
            update_band_control(Some((
                UpdateBandControlPayload {
                    group: g as u8,
                    key: k as u8,
                    value: normalized_value as f32,
                },
                (),
            )));
        }
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
        let k1_tx = sun_screen_x - (key_x() + key_width() / 2.0);
        let k1_ty = sun_screen_y - (key_y() + key_height() / 2.0);

        // Breadth-first move: fix the axis orthogonal to main
        let (breadth_tx, breadth_ty) = match orientation {
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

    // Classes with one-shot appear animation
    let key_wrapper_class = move || {
        let base = "relative";
        if key_should_animate() {
            format!(
                "{} {}",
                super::instrument_animations::INSTRUMENT_KEY_APPEAR,
                base
            )
        } else {
            base.to_string()
        }
    };
    let band_class = move || {
        let base = "absolute rounded-full bg-red/90 dark:bg-black/90 border-(length:--keyboard-band-stroke-width) border-black dark:border-red backdrop-blur-xl";
        if band_should_animate() {
            format!(
                "{} {}",
                super::instrument_animations::INSTRUMENT_BAND_APPEAR,
                base
            )
        } else {
            base.to_string()
        }
    };

    let channel_alignment = match first_group_channel.nth_channel_from_first(g) {
        common::instrument::GroupChannel::Left => {
            r#"
            top: 0;
            left: 0;
            "#
        }
        common::instrument::GroupChannel::Right => {
            r#"
            bottom: 0;
            right: 0;
            "#
        }
    };

    // Orientation-dependent geometry CSS for the band
    let band_geom_style = match orientation {
        common::orientation::LayoutOrientation::Vertical => {
            format!(
                r#"
                        width: var(--keyboard-band-length);
                        height: var(--keyboard-band-breadth);
                        {channel_alignment}
                        "#
            )
        }
        common::orientation::LayoutOrientation::Horizontal => {
            format!(
                r#"
                        width: var(--keyboard-band-breadth);
                        height: var(--keyboard-band-length);
                        {channel_alignment}
                        "#
            )
        }
    };
    // Bands move with their wrapper; only glare scales are per-element
    let band_scale_vars = move || {
        // Reverse glare sequencing: deeper (background) bands start earlier,
        // foreground bands later, to respect stacking order.
        let total = num_keys_per_group() as f32;
        let glare_index = (total - 1.0 - k as f32).max(0.0);
        let glare_start = 0.75 + 0.05 * glare_index;
        let glare_peak = 1.5 + 0.1 * glare_index;
        format!(
            concat!(
                "--inst-band-glare-scale: {};",
                " --inst-band-glare-scale-peak: {};"
            ),
            glare_start, glare_peak
        )
    };
    let band_style = move || {
        let mut s = String::new();
        s.push_str(&band_scale_vars());
        s.push_str(&band_geom_style);
        s
    };

    Effect::new(move |prev: Option<common::instrument::Layout>| {
        let complete_layout = complete_layout();
        if prev.is_none_or(|old_layout| old_layout != complete_layout) {
            band_update();
            key_update();
        }
        complete_layout
    });

    view! {
        <div class=key_wrapper_class node_ref=key_ref style=key_stage1_vars>
            <div
                class=band_class
                node_ref=band_ref
                id=format!("key-band-{g}-{k}")
                role="presentation"
                style=band_style
            ></div>
            <Button
                class=Signal::derive(move || {
                    let ring = if is_pressed() {
                        "inset-ring-3 inset-ring-cinnabar dark:inset-ring-gray ring-2 ring-gray dark:ring-cinnabar"
                    } else {
                        ""
                    };
                    format!(
                        "border-none text-thin md:text-base text-sm absolute {} {} will-change-[transform, top, left]",
                        if is_dragging() { "cursor-grabbing" } else { "cursor-grab" },
                        ring,
                    )
                })
                size=UiSize::Sm
                round=true
                square=true
                attr:id=format!("key-{g}-{k}")
                on:click=move |ev| {
                    ev.prevent_default();
                    ev.stop_propagation();
                    let new_value = if is_pressed() { 0.0 } else { 1.0 };
                    is_pressed.set(!is_pressed());
                    update_key_control(
                        Some((
                            UpdateKeyControlPayload {
                                group: g as u8,
                                key: k as u8,
                                value: new_value,
                            },
                            (),
                        )),
                    );
                }
                attr:aria-pressed=move || is_pressed().to_string()
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
                    let inc = samples.iter().map(|v| v.abs()).sum::<f32>();
                    format!("scale({s}, {s})", s = 0.75 + (inc / samples.len() as f32) * 0.275)
                }
                style:top=move || { format!("{}px", constrained_position().y) }
                style:left=move || { format!("{}px", constrained_position().x) }
                node_ref=draggable_ref
            >
                {""}
            </Button>
        </div>
    }
}
