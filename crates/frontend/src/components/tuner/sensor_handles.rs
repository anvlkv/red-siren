use std::collections::HashMap;

use common::{
    tuner::{Config, Layout as TunerLayout, UpdateSensorPayload},
    NodeKey,
};
use leptos::{callback::Callback, ev};
use leptos::{html, prelude::*};
use leptos_use::{use_event_listener, use_window};
use mint::Point2;
use web_sys::CanvasRenderingContext2d;
use web_time::Instant;

use crate::util::drawing::{get_2d_ctx, is_dark_mode, resolve_theme_color};

type Color = (u8, u8, u8);
type Highlight<'a> = (&'a str, f64, f64);
type Object = (NodeKey, Option<SensorArmDirection>);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SensorArmDirection {
    Min,
    Max,
}

/// Collection of sensor handles
#[component]
pub fn SensorHandles(
    /// Configuration signal
    #[prop(into)]
    config: Signal<Config>,
    /// Tuner layout signal
    #[prop(into)]
    layout: Signal<TunerLayout>,
    /// Unified canvas transform provided by the parent Tuner
    #[prop(into)]
    canvas_transform: Signal<super::CanvasTransform>,
    /// Callback for updating sensors
    #[prop(into)]
    on_update: Callback<UpdateSensorPayload>,
) -> impl IntoView {
    // let context = expect_tuner_service();
    let canvas_ref = NodeRef::<html::Canvas>::new();
    let picking_canvas_ref = NodeRef::<html::Canvas>::new();
    let cursor_canvas_ref = NodeRef::<html::Canvas>::new();

    let is_dark = is_dark_mode();

    let base_color = Memo::new(move |_| {
        let base_var = if is_dark() {
            "--color-red"
        } else {
            "--color-black"
        };
        resolve_theme_color(base_var).unwrap_or_else(|| {
            if is_dark() {
                "#e30022".into()
            } else {
                "#353839".into()
            }
        })
    });

    let secondary_color = Memo::new(move |_| {
        let base_var = if is_dark() {
            "--color-cinnabar"
        } else {
            "--color-gray"
        };
        resolve_theme_color(base_var).unwrap_or_else(|| {
            if is_dark() {
                "#e44d2e".into()
            } else {
                "#36454f".into()
            }
        })
    });

    // Setup canvas backing resolution and scaling whenever canvas mounts or transform changes
    Effect::new(move |_| {
        if let Some(((canvas, picking_canvas), cursor_canvas)) = canvas_ref
            .get()
            .zip(picking_canvas_ref.get())
            .zip(cursor_canvas_ref.get())
        {
            let tr = canvas_transform();
            // Set backing resolution in device pixels (provided by parent)
            let bw = tr.backing_width_dev;
            let bh = tr.backing_height_dev;
            canvas.set_width(bw);
            canvas.set_height(bh);
            picking_canvas.set_width(bw);
            picking_canvas.set_height(bh);
            cursor_canvas.set_width(bw);
            cursor_canvas.set_height(bh);

            // Apply uniform scale with letterboxing/pillarboxing using provided transform
            if let Some(ctx) = get_2d_ctx(&canvas) {
                _ = ctx.reset_transform().ok();
                let tx_world = tr.translate_x_margin_dev / tr.device_scale_x;
                let ty_world = tr.translate_y_margin_dev / tr.device_scale_y;
                _ = ctx.translate(tx_world, ty_world).ok();
                _ = ctx.scale(tr.device_scale_x, tr.device_scale_y).ok();
            }

            if let Some(picking_ctx) = get_2d_ctx(&picking_canvas) {
                _ = picking_ctx.reset_transform().ok();
                let tx_world = tr.translate_x_margin_dev / tr.device_scale_x;
                let ty_world = tr.translate_y_margin_dev / tr.device_scale_y;
                _ = picking_ctx.translate(tx_world, ty_world).ok();
                _ = picking_ctx.scale(tr.device_scale_x, tr.device_scale_y).ok();
            }

            if let Some(cursor_ctx) = get_2d_ctx(&cursor_canvas) {
                _ = cursor_ctx.reset_transform().ok();
                let tx_world = tr.translate_x_margin_dev / tr.device_scale_x;
                let ty_world = tr.translate_y_margin_dev / tr.device_scale_y;
                _ = cursor_ctx.translate(tx_world, ty_world).ok();
                _ = cursor_ctx.scale(tr.device_scale_x, tr.device_scale_y).ok();
            }
        }
    });

    let object_map = StoredValue::new(HashMap::<Color, Object>::new());
    let highlighted_object = RwSignal::new(None::<Object>);
    let selected_objects = RwSignal::new(HashMap::<i32, (Object, Point2<f64>, Instant)>::new());

    Effect::new(move |_| {
        let lay = layout();
        let config = config();
        let radius = lay.sensor_radius;
        let highlighted_object = highlighted_object();
        let base_color = base_color();
        let secondary_color = secondary_color();
        let selection = selected_objects.get();

        object_map.update_value(|object_map| {
            if let Some((ctx, picking_ctx)) =
                canvas_ref.get().and_then(|canvas| get_2d_ctx(&canvas)).zip(
                    picking_canvas_ref
                        .get()
                        .and_then(|canvas| get_2d_ctx(&canvas)),
                )
            {
                // Clear full canvas (in device pixels), independent of current transform
                let tr = canvas_transform();
                let bw = tr.backing_width_dev as f64;
                let bh = tr.backing_height_dev as f64;
                ctx.save();
                _ = ctx.reset_transform().ok();
                ctx.clear_rect(0.0, 0.0, bw, bh);
                ctx.restore();
                picking_ctx.save();
                _ = picking_ctx.reset_transform().ok();
                picking_ctx.clear_rect(0.0, 0.0, bw, bh);
                picking_ctx.restore();

                // draw sensors
                for sensor in config.sensor_data.iter() {
                    let (main_picking_color, min_picking_color, max_picking_color) = {
                        let main_obj = (sensor.key, None);
                        let min_obj = (sensor.key, Some(SensorArmDirection::Min));
                        let max_obj = (sensor.key, Some(SensorArmDirection::Max));
                        let (main_picking_color, min_picking_color, max_picking_color) = (
                            picking_color_for_object(main_obj),
                            picking_color_for_object(min_obj),
                            picking_color_for_object(max_obj),
                        );

                        if !object_map.contains_key(&picking_color_for_object(main_obj)) {
                            object_map.insert(main_picking_color, main_obj);
                            object_map.insert(min_picking_color, min_obj);
                            object_map.insert(max_picking_color, max_obj);
                        };

                        (
                            color_to_string(main_picking_color),
                            color_to_string(min_picking_color),
                            color_to_string(max_picking_color),
                        )
                    };

                    let start = config.frequency_magnitude_to_space(
                        &lay,
                        sensor.min_frequency,
                        sensor.min_magnitude,
                    );
                    let end = config.frequency_magnitude_to_space(
                        &lay,
                        sensor.max_frequency,
                        sensor.max_magnitude,
                    );

                    draw_connector(start, end, &main_picking_color, radius, None, &picking_ctx);
                    draw_sensor_handle(
                        SensorArmDirection::Min,
                        start,
                        radius,
                        &min_picking_color,
                        &min_picking_color,
                        None,
                        lay.orientation,
                        &picking_ctx,
                    );
                    draw_sensor_handle(
                        SensorArmDirection::Max,
                        end,
                        radius,
                        &max_picking_color,
                        &max_picking_color,
                        None,
                        lay.orientation,
                        &picking_ctx,
                    );

                    let highlight = highlighted_object
                        .as_ref()
                        .filter(|(k, _)| k == &sensor.key);

                    draw_connector(
                        start,
                        end,
                        &secondary_color,
                        0.5,
                        highlight.map(|o| {
                            highlight_color(None, o, &selection, &base_color, &secondary_color)
                        }),
                        &ctx,
                    );
                    draw_sensor_handle(
                        SensorArmDirection::Min,
                        start,
                        radius,
                        &base_color,
                        &secondary_color,
                        highlight.map(|o| {
                            highlight_color(
                                Some(SensorArmDirection::Min),
                                o,
                                &selection,
                                &base_color,
                                &secondary_color,
                            )
                        }),
                        lay.orientation,
                        &ctx,
                    );
                    draw_sensor_handle(
                        SensorArmDirection::Max,
                        end,
                        radius,
                        &base_color,
                        &secondary_color,
                        highlight.map(|o| {
                            highlight_color(
                                Some(SensorArmDirection::Max),
                                o,
                                &selection,
                                &base_color,
                                &secondary_color,
                            )
                        }),
                        lay.orientation,
                        &ctx,
                    );
                }
            }
        });
    });

    let freq_mag_space = Memo::new(move |_| {
        let config = config();
        let lay = layout();

        // Point A: baseline start
        let a = Point2 {
            x: lay.line_position.0.x,
            y: lay.line_position.0.y,
        };

        // Point B: extreme along the magnitude and frequency axes depending on orientation
        let b = match lay.orientation {
            common::orientation::LayoutOrientation::Horizontal => Point2 {
                x: lay.line_position.1.x, // x max frequency
                y: 0.0,                   // top edge (max magnitude)
            },
            common::orientation::LayoutOrientation::Vertical => Point2 {
                x: lay.space.x,           // right edge (max magnitude)
                y: lay.line_position.1.y, // y max frequency
            },
        };

        let (a_freq, a_mag) = config.space_to_frequency_magnitude(&lay, a);
        let (b_freq, b_mag) = config.space_to_frequency_magnitude(&lay, b);

        (
            (a_freq as f64)..=(b_freq as f64),
            (a_mag as f64)..=(b_mag as f64),
        )
    });

    let object_under_pointer = move |x_css: f64, y_css: f64| -> Option<Object> {
        let tr = canvas_transform();
        let (dx, dy) = tr.css_to_device(x_css, y_css);
        let dx = dx.floor();
        let dy = dy.floor();
        picking_canvas_ref
            .get()
            .and_then(|canvas| get_2d_ctx(&canvas))
            .and_then(|ctx| {
                let img = ctx.get_image_data(dx, dy, 1.0, 1.0).ok();
                img.and_then(|data| {
                    let data = data.data();
                    if data.len() >= 3 {
                        Some((data[0], data[1], data[2]))
                    } else {
                        None
                    }
                })
            })
            .and_then(|pixel| {
                let object_map = object_map.get_value();
                object_map.get(&pixel).copied()
            })
    };

    let cross_hair = move |x_css: f64, y_css: f64| {
        let color = base_color();
        if let Some(ctx) = cursor_canvas_ref
            .get()
            .and_then(|canvas| get_2d_ctx(&canvas))
        {
            // Clear entire backing store in device pixels, independent of transform
            let tr = canvas_transform();
            let bw = tr.backing_width_dev as f64;
            let bh = tr.backing_height_dev as f64;
            ctx.save();
            _ = ctx.reset_transform().ok();
            ctx.clear_rect(0.0, 0.0, bw, bh);
            ctx.restore();

            // Convert CSS pointer to world-space under current transform
            let (world_x, world_y) = tr.css_to_world(x_css, y_css);

            // Draw crosshair at world coordinates
            draw_cross_hair(world_x, world_y, 10.0, &color, &ctx);
        }
    };

    let unlisten_pointerout =
        use_event_listener(use_window(), ev::pointerout, move |ev: ev::PointerEvent| {
            let id = ev.pointer_id();
            selected_objects.update(|selected| {
                selected.remove(&id);
            });
        });

    let on_move = move |ev: ev::PointerEvent| {
        let x = ev.offset_x() as f64;
        let y = ev.offset_y() as f64;

        cross_hair(x, y);

        let id = ev.pointer_id();
        let selected = selected_objects();
        if let Some(((key, part), mut point, inst)) = selected.get(&id).cloned() {
            let (freq_space, mag_space) = freq_mag_space();
            let freq_range = freq_space.end() - freq_space.start();
            let mag_range = mag_space.end() - mag_space.start();
            let lay = layout();

            // Deltas in CSS px (match world units since canvas CSS equals layout space)
            let d_x = x - point.x;
            let d_y = y - point.y;

            // World-space deltas
            let d_x_world = d_x;
            let d_y_world = d_y;

            let now = Instant::now();
            let dur = (now.duration_since(inst).as_secs_f64()).max(1e-3);

            // Velocity in world units per second
            let v0_x = lay.space.x;
            let v0_y = lay.space.y;
            let v_x = d_x_world.abs() / dur;
            let v_y = d_y_world.abs() / dur;

            // Fractional movement relative to full space with smoothing
            let scale_x = if v0_x > 0.0 { v_x / (v_x + v0_x) } else { 1.0 };
            let scale_y = if v0_y > 0.0 { v_y / (v_y + v0_y) } else { 1.0 };
            let frac_x = ((d_x_world / v0_x) * scale_x).clamp(-1.0, 1.0);
            let frac_y = ((d_y_world / v0_y) * scale_y).clamp(-1.0, 1.0);

            let ctrl = ev.ctrl_key();
            let alt = ev.alt_key();

            let (mag_increment, freq_increment) = match layout().orientation {
                common::orientation::LayoutOrientation::Horizontal => {
                    // x controls frequency, y controls magnitude
                    let freq_inc = (freq_range * frac_x) as f32;
                    let mag_inc = (-mag_range * frac_y) as f32;
                    if alt {
                        if d_x.abs() > d_y.abs() {
                            (0.0, freq_inc)
                        } else {
                            (mag_inc, 0.0)
                        }
                    } else {
                        (mag_inc, freq_inc)
                    }
                }
                common::orientation::LayoutOrientation::Vertical => {
                    // y controls frequency, x controls magnitude
                    let freq_inc = (freq_range * frac_y) as f32;
                    let mag_inc = (mag_range * frac_x) as f32;
                    if alt {
                        if d_y.abs() > d_x.abs() {
                            (0.0, freq_inc)
                        } else {
                            (mag_inc, 0.0)
                        }
                    } else {
                        (mag_inc, freq_inc)
                    }
                }
            };

            let keys = if ctrl {
                config()
                    .sensor_data
                    .iter()
                    .map(|s| s.key)
                    .collect::<Vec<_>>()
            } else {
                vec![key]
            };

            let update_payload = match part {
                None => UpdateSensorPayload {
                    keys,
                    min_frequency_increment: freq_increment,
                    max_frequency_increment: freq_increment,
                    min_magnitude_increment: mag_increment,
                    max_magnitude_increment: mag_increment,
                },
                Some(SensorArmDirection::Min) => UpdateSensorPayload {
                    keys,
                    min_frequency_increment: freq_increment,
                    max_frequency_increment: 0.0,
                    min_magnitude_increment: mag_increment,
                    max_magnitude_increment: 0.0,
                },
                Some(SensorArmDirection::Max) => UpdateSensorPayload {
                    keys,
                    max_frequency_increment: freq_increment,
                    min_frequency_increment: 0.0,
                    max_magnitude_increment: mag_increment,
                    min_magnitude_increment: 0.0,
                },
            };

            on_update.run(update_payload);

            point.x = x;
            point.y = y;
            selected_objects.update(|objects| {
                let entry = objects.entry(id);
                entry.and_modify(|v| {
                    v.1 = point;
                    v.2 = now;
                });
            });
        } else {
            let object_under_pointer = object_under_pointer(x, y);
            highlighted_object.set(object_under_pointer);
        }
    };

    let on_down = move |ev: ev::PointerEvent| {
        if let Some(_canvas) = cursor_canvas_ref.get() {
            let x = ev.offset_x() as f64;
            let y = ev.offset_y() as f64;
            let id = ev.pointer_id();
            if let Some(object_under_pointer) = object_under_pointer(x, y) {
                selected_objects.update(|selected| {
                    selected.insert(id, (object_under_pointer, Point2 { x, y }, Instant::now()));
                });
            }
        }
    };

    let on_up = move |ev: ev::PointerEvent| {
        let id = ev.pointer_id();
        selected_objects.update(|selected| {
            selected.remove(&id);
        });
    };

    on_cleanup(move || {
        unlisten_pointerout();
    });

    view! {
        <>
            <canvas
                class="absolute inset-0 w-full h-full pointer-events-none invisible pointer-events-none"
                node_ref=picking_canvas_ref
            />
            <canvas
                class="absolute inset-0 w-full h-full mix-blend-difference pointer-events-none"
                node_ref=canvas_ref
            />
            <canvas
                class="absolute inset-0 w-full h-full mix-blend-exclusion"
                on:pointermove=on_move
                on:pointerdown=on_down
                on:pointerup=on_up
                style:cursor=move || {
                    if highlighted_object().is_some() && selected_objects().is_empty() {
                        "grab"
                    } else if !selected_objects().is_empty() {
                        "grabbing"
                    } else {
                        "default"
                    }
                }
                node_ref=cursor_canvas_ref
            />
        </>
    }
}

fn draw_cross_hair(
    center_x: f64,
    center_y: f64,
    length: f64,
    color: &str,
    ctx: &CanvasRenderingContext2d,
) {
    ctx.save();
    ctx.set_stroke_style_str(color);
    ctx.set_line_width(1.0);
    ctx.begin_path();
    // Horizontal line
    ctx.move_to(center_x - length / 2.0, center_y);
    ctx.line_to(center_x + length / 2.0, center_y);
    // Vertical line
    ctx.move_to(center_x, center_y - length / 2.0);
    ctx.line_to(center_x, center_y + length / 2.0);
    ctx.stroke();
    ctx.restore();
}

#[allow(clippy::too_many_arguments)]
fn draw_sensor_handle(
    direction: SensorArmDirection,
    center: Point2<f64>,
    radius: f64,
    fill_color: &str,
    stroke_color: &str,
    highlight: Option<Highlight>,
    orientation: common::orientation::LayoutOrientation,
    ctx: &CanvasRenderingContext2d,
) {
    let path = || match (direction, orientation) {
        (SensorArmDirection::Min, common::orientation::LayoutOrientation::Vertical) => {
            ctx.move_to(center.x, center.y);
            ctx.arc(center.x, center.y, radius, 0.0, std::f64::consts::PI)
                .unwrap();
            ctx.line_to(center.x, center.y);
        }
        (SensorArmDirection::Min, common::orientation::LayoutOrientation::Horizontal) => {
            ctx.move_to(center.x, center.y);
            ctx.arc(
                center.x,
                center.y,
                radius,
                std::f64::consts::FRAC_PI_2,
                std::f64::consts::FRAC_PI_2 + std::f64::consts::PI,
            )
            .unwrap();
            ctx.line_to(center.x, center.y);
        }
        (SensorArmDirection::Max, common::orientation::LayoutOrientation::Vertical) => {
            ctx.move_to(center.x, center.y);
            ctx.arc(center.x, center.y, radius, std::f64::consts::PI, 0.0)
                .unwrap();
            ctx.line_to(center.x, center.y);
        }
        (SensorArmDirection::Max, common::orientation::LayoutOrientation::Horizontal) => {
            ctx.move_to(center.x, center.y);
            ctx.arc(
                center.x,
                center.y,
                radius,
                std::f64::consts::FRAC_PI_2 + std::f64::consts::PI,
                std::f64::consts::FRAC_PI_2,
            )
            .unwrap();
            ctx.line_to(center.x, center.y);
        }
    };

    if let Some((highlight_color, alpha, spread)) = highlight {
        ctx.save();
        // Outer glow (lighter, blurred)
        ctx.set_global_alpha(alpha);
        ctx.set_stroke_style_str(highlight_color);
        ctx.set_line_width(1.5);
        ctx.set_shadow_color(highlight_color);
        ctx.set_shadow_blur(spread);
        ctx.set_shadow_offset_x(0.0);
        ctx.set_shadow_offset_y(0.0);
        ctx.begin_path();
        path();
        ctx.stroke();

        // Inner ring (solid, most prominent for shape highlight)
        ctx.set_global_alpha(1.0);
        ctx.set_shadow_blur(0.0);
        ctx.set_shadow_offset_x(0.0);
        ctx.set_shadow_offset_y(0.0);
        ctx.set_line_width(2.0);
        ctx.set_stroke_style_str(highlight_color);
        ctx.begin_path();
        path();
        ctx.stroke();

        ctx.restore();
    }

    ctx.save();
    ctx.set_stroke_style_str(stroke_color);
    ctx.set_fill_style_str(fill_color);
    ctx.set_line_width(0.5);
    ctx.begin_path();
    path();
    ctx.fill();
    ctx.stroke();
    ctx.restore();
}

fn draw_connector(
    p1: Point2<f64>,
    p2: Point2<f64>,
    color: &str,
    thickness: f64,
    highlight: Option<Highlight>,
    ctx: &CanvasRenderingContext2d,
) {
    if let Some((highlight_color, alpha, spread)) = highlight {
        ctx.save();
        // Outer glow (lighter, blurred)
        ctx.set_global_alpha(alpha);
        ctx.set_stroke_style_str(highlight_color);
        // Boost width if this is a selected part (alpha ~ 1.0 from mapping)
        let wide = if alpha >= 0.99 {
            thickness * 4.0
        } else {
            thickness * 3.0
        };
        ctx.set_line_width(wide);
        ctx.set_shadow_color(highlight_color);
        ctx.set_shadow_blur(spread);
        ctx.set_shadow_offset_x(0.0);
        ctx.set_shadow_offset_y(0.0);
        ctx.begin_path();
        ctx.move_to(p1.x, p1.y);
        ctx.line_to(p2.x, p2.y);
        ctx.stroke();

        // Inner ring (solid, most prominent for selected-part)
        ctx.set_global_alpha(1.0);
        ctx.set_shadow_blur(0.0);
        ctx.set_shadow_offset_x(0.0);
        ctx.set_shadow_offset_y(0.0);
        ctx.set_line_width(thickness * 2.0);
        ctx.set_stroke_style_str(highlight_color);
        ctx.begin_path();
        ctx.move_to(p1.x, p1.y);
        ctx.line_to(p2.x, p2.y);
        ctx.stroke();

        ctx.restore();
    }
    ctx.save();
    ctx.set_stroke_style_str(color);
    ctx.set_line_width(thickness);
    ctx.begin_path();
    ctx.move_to(p1.x, p1.y);
    ctx.line_to(p2.x, p2.y);
    ctx.stroke();
    ctx.restore();
}

fn picking_color_for_object((key, direction): Object) -> Color {
    let idx = key.idx() as u32; // fits in 24 bits
    let dir_val: u32 = match direction {
        Some(SensorArmDirection::Min) => 1,
        Some(SensorArmDirection::Max) => 2,
        None => 0,
    };
    let id = (idx << 2) | dir_val;

    // Base packing
    let r0 = ((id >> 16) & 0xFF) as u8;
    let g0 = ((id >> 8) & 0xFF) as u8;
    let b0 = (id & 0xFF) as u8;

    // Distinct, invertible transform per channel:
    // rotate left by k bits then XOR with a mask
    let r = r0.rotate_left(1) ^ 0xA5;
    let g = g0.rotate_left(3) ^ 0x5A;
    let b = b0.rotate_left(5) ^ 0x3C;

    (r, g, b)
}

fn highlight_color<'a>(
    part: Option<SensorArmDirection>,
    (key, highlight_part): &Object,
    selection: &HashMap<i32, (Object, Point2<f64>, Instant)>,
    base_color: &'a str,
    secondary_color: &'a str,
) -> Highlight<'a> {
    let (is_selected, is_selected_part) = selection
        .values()
        .find_map(|((k, p), _, _)| {
            let is_same = k == key;
            if is_same {
                let is_same_part = p == &part;
                Some((is_same, is_same_part))
            } else {
                None
            }
        })
        .unwrap_or((false, false));
    let is_highlighted_part = &part == highlight_part;

    match (is_selected, is_selected_part, is_highlighted_part) {
        // Generic highlight -> least prominent
        (false, _, false) => (secondary_color, 0.4, 3.0),
        // Highlighted part (not selected) -> more prominent than generic highlight
        (false, _, true) => (secondary_color, 0.7, 5.0),
        // Selected (whole) -> second most prominent
        (true, false, _) => (base_color, 0.85, 6.0),
        // Selected part -> most prominent
        (true, true, _) => (base_color, 1.0, 8.0),
    }
}

fn color_to_string((r, g, b): Color) -> String {
    format!("rgb({},{},{})", r, g, b)
}
