use leptos::{html, prelude::*};
use tauri_use::{use_command, UseTauriWithReturn};
use web_sys::CanvasRenderingContext2d;

use common::orientation::LayoutOrientation;
use common::tuner::{Config, Layout as TunerLayout, SpectrumData};

use crate::components::expect_tuner_service;
use crate::util::drawing::{get_2d_ctx, is_dark_mode, resolve_theme_color};
use crate::util::raf_fn_fps::use_raf_fn_with_fps;

/// Spectrum visualizer component for displaying FFT analysis results using HTML Canvas
#[component]
pub fn SpectrumVisualizer(
    /// Tuner layout (space, orientation, baseline, sensors count)
    #[prop(into)]
    layout: Signal<Option<TunerLayout>>,
    /// Unified canvas transform provided by the parent Tuner
    #[prop(into)]
    canvas_transform: Signal<super::CanvasTransform>,
) -> impl IntoView {
    let context = expect_tuner_service();
    let canvas_ref = NodeRef::<html::Canvas>::new();

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
        if let Some(canvas) = canvas_ref.get() {
            // Set backing resolution in device pixels (provided by parent)
            let tr = canvas_transform();
            canvas.set_width(tr.backing_width_dev);
            canvas.set_height(tr.backing_height_dev);

            // Apply uniform scale with letterboxing/pillarboxing (provided transform)
            if let Some(ctx) = get_2d_ctx(&canvas) {
                _ = ctx.reset_transform().ok();
                // Apply margin-aware translate in world units, then per-axis scale
                let tx_world = tr.translate_x_margin_dev / tr.device_scale_x;
                let ty_world = tr.translate_y_margin_dev / tr.device_scale_y;
                _ = ctx.translate(tx_world, ty_world).ok();
                _ = ctx.scale(tr.device_scale_x, tr.device_scale_y).ok();
            }
        }
    });

    let UseTauriWithReturn {
        data: spectrum_data,
        error: spectrum_error,
        trigger: poll_spectrum,
    } = use_command::<SpectrumData>(common::commands::tuner::SPECTRUM_DATA);

    let UseTauriWithReturn {
        data: input_data,
        error: input_error,
        trigger: poll_input,
    } = use_command::<Vec<f32>>(common::commands::tuner::INPUT_SNOOP);

    _ = use_raf_fn_with_fps(
        move |_| {
            poll_spectrum(Some(()));
            poll_input(Some(()));
            if let Some(spectrum) = spectrum_data() {
                let Some(lay) = layout() else { return };
                let Some(ctx) = canvas_ref.get().and_then(|canvas| get_2d_ctx(&canvas)) else {
                    return;
                };
                let input_data = input_data().unwrap_or_default();

                // Clear full canvas (in device pixels), independent of current transform
                let tr = canvas_transform();
                let bw = tr.backing_width_dev as f64;
                let bh = tr.backing_height_dev as f64;
                ctx.save();
                _ = ctx.reset_transform().ok();
                ctx.clear_rect(0.0, 0.0, bw, bh);
                ctx.restore();

                let base_color = base_color.get();
                let secondary_color = secondary_color.get();

                // Configure blend to match original SVG classes intent
                // We mimic mix-blend-plus-darker/lighter with globalCompositeOperation hints.
                // Fallback to "source-over" if unsupported.
                _ = ctx.set_global_composite_operation("source-over");

                // Draw baseline reference line
                draw_input_line(&ctx, &lay, &tr, &input_data, &base_color);

                // Draw max-hold sensor excitement outline bars (lower opacity strokes)
                draw_excitement_bars(
                    &ctx,
                    &lay,
                    &tr,
                    &spectrum.max_excitements,
                    &base_color,
                    &secondary_color,
                    0.40,
                    true,
                );

                // Draw sensor excitement bars (filled, higher opacity)
                draw_excitement_bars(
                    &ctx,
                    &lay,
                    &tr,
                    &spectrum.sensor_excitements,
                    &base_color,
                    &secondary_color,
                    0.30,
                    false,
                );

                // Draw spectrum layers
                let cfg = context.config.get().unwrap_or_default();

                if !spectrum.max_magnitudes.is_empty() && !spectrum.frequencies.is_empty() {
                    draw_spectrum_area(
                        &ctx,
                        &cfg,
                        &tr,
                        &lay,
                        &spectrum.max_magnitudes,
                        &spectrum.frequencies,
                        0.30, // alpha to match "fill-gray/30 ... stroke-.../50"
                        &base_color,
                        &secondary_color,
                    );
                }

                if !spectrum.current_magnitudes.is_empty() && !spectrum.frequencies.is_empty() {
                    draw_spectrum_area(
                        &ctx,
                        &cfg,
                        &tr,
                        &lay,
                        &spectrum.current_magnitudes,
                        &spectrum.frequencies,
                        0.60, // alpha to match "fill-gray/40 ... stroke-.../60"
                        &base_color,
                        &secondary_color,
                    );
                }

                context.spectrum.set(Some(spectrum));
            }
        },
        5.0,
    );

    Effect::new(move |_| {
        if let Some(err) = spectrum_error() {
            log::error!("Error listening to spectrum data: {err}");
        }

        if let Some(err) = input_error() {
            log::error!("Error listening to input snoop data: {err}");
        }
    });

    view! {
        <canvas
            node_ref=canvas_ref
            style:touch-action="none"
            class="absolute inset-0 w-full h-full"
        ></canvas>
    }
}

fn draw_input_line(
    ctx: &CanvasRenderingContext2d,
    layout: &TunerLayout,
    tr: &super::CanvasTransform,
    input_data: &[f32],
    base_color: &str,
) {
    ctx.save();
    ctx.set_stroke_style_str(base_color);
    ctx.set_line_width(1.0);

    let baseline = layout.line_position;
    let input_level = layout
        .orientation
        .safe_breadth(layout.space, layout.safe_area_padding);
    let mid_level = input_level / 2.0;

    let n = input_data.len();
    if n == 0 {
        ctx.restore();
        return;
    }

    // Vector along the baseline from start to end
    let dx = baseline.1.x - baseline.0.x;
    let dy = baseline.1.y - baseline.0.y;

    ctx.begin_path();

    for (i, &val) in input_data.iter().enumerate() {
        // Interpolate along the baseline
        let t = if n == 1 {
            0.0
        } else {
            i as f64 / (n - 1) as f64
        };
        let bx = baseline.0.x + dx * t;
        let by = baseline.0.y + dy * t;

        // Perpendicular deflection based on orientation (signed amplitude in range -1..1)
        let amp = (val as f64) * mid_level;

        let (x, y) = match layout.orientation {
            LayoutOrientation::Vertical => (bx + amp, by),
            LayoutOrientation::Horizontal => (bx, by - amp),
        };

        if i == 0 {
            ctx.move_to(x, y);
        } else {
            ctx.line_to(x, y);
        }
    }

    ctx.stroke();

    // Extend the input line under overlay margins so overlays have content underneath.
    // Convert overlay margins to world units using the same transform applied to the canvas.
    let left_world = tr.translate_x_margin_dev / tr.device_scale_x;
    let right_world = (tr.margin_right_css * tr.device_pixel_ratio) / tr.device_scale_x;
    let top_world = tr.translate_y_margin_dev / tr.device_scale_y;
    let bottom_world = (tr.margin_bottom_css * tr.device_pixel_ratio) / tr.device_scale_y;

    // First and last computed points of the input line
    let first_t = 0.0;
    let last_t = 1.0;
    let bx0 = baseline.0.x + dx * first_t;
    let by0 = baseline.0.y + dy * first_t;
    let bx1 = baseline.0.x + dx * last_t;
    let by1 = baseline.0.y + dy * last_t;
    let amp0 = (input_data[0] as f64) * mid_level;
    let amp1 = (input_data[n - 1] as f64) * mid_level;

    match layout.orientation {
        LayoutOrientation::Horizontal => {
            // Compute first/last points with vertical deflection
            let p0 = (bx0, by0 - amp0);
            let p1 = (bx1, by1 - amp1);

            // Left bar: extend first sample into negative X by left_world
            if left_world > 0.0 {
                ctx.begin_path();
                ctx.move_to(0.0, by0);
                ctx.line_to(0.0, p0.1);
                ctx.line_to(-left_world, p0.1);
                ctx.line_to(-left_world, by0);
                ctx.close_path();
                ctx.fill();
                ctx.stroke();
            }
            // Right bar: extend last sample beyond space.x by right_world
            if right_world > 0.0 {
                ctx.begin_path();
                ctx.move_to(layout.space.x, by1);
                ctx.line_to(layout.space.x, p1.1);
                ctx.line_to(layout.space.x + right_world, p1.1);
                ctx.line_to(layout.space.x + right_world, by1);
                ctx.close_path();
                ctx.fill();
                ctx.stroke();
            }
        }
        LayoutOrientation::Vertical => {
            // Compute first/last points with horizontal deflection
            let p0 = (bx0 + amp0, by0);
            let p1 = (bx1 + amp1, by1);

            // Top bar: extend first sample upward (negative Y) by top_world
            if top_world > 0.0 {
                ctx.begin_path();
                ctx.move_to(bx0, 0.0);
                ctx.line_to(p0.0, 0.0);
                ctx.line_to(p0.0, -top_world);
                ctx.line_to(bx0, -top_world);
                ctx.close_path();
                ctx.fill();
                ctx.stroke();
            }
            // Bottom bar: extend last sample beyond space.y by bottom_world
            if bottom_world > 0.0 {
                ctx.begin_path();
                ctx.move_to(bx1, layout.space.y);
                ctx.line_to(p1.0, layout.space.y);
                ctx.line_to(p1.0, layout.space.y + bottom_world);
                ctx.line_to(bx1, layout.space.y + bottom_world);
                ctx.close_path();
                ctx.fill();
                ctx.stroke();
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_excitement_bars(
    ctx: &CanvasRenderingContext2d,
    layout: &TunerLayout,
    tr: &super::CanvasTransform,
    excitements: &[f32],
    base_color: &str,
    secondary_color: &str,
    alpha: f64,
    is_stroke: bool,
) {
    let baseline = layout.line_position;
    ctx.save();
    if is_stroke {
        ctx.set_stroke_style_str(secondary_color);
        ctx.set_line_width(0.5);
    } else {
        ctx.set_fill_style_str(base_color);
    }
    ctx.set_global_alpha(alpha);

    let num_sensors = layout.num_sensors.get() as usize;
    if num_sensors == 0 {
        ctx.restore();
        return;
    }

    let line_len = match layout.orientation {
        LayoutOrientation::Horizontal => (baseline.1.x - baseline.0.x).abs(),
        LayoutOrientation::Vertical => (baseline.1.y - baseline.0.y).abs(),
    };
    let bar_w = line_len / num_sensors as f64;
    let spacing = bar_w * 0.1;

    let scale = {
        let max_act = excitements
            .iter()
            .copied()
            .fold(0.0f32, |acc, v| acc.max(v.abs()));
        if max_act > 1.0 {
            1.0 / max_act
        } else {
            1.0
        }
    } as f64;

    for (i, &exc) in excitements.iter().enumerate() {
        let lvl = (exc as f64 * scale).clamp(0.0, 1.0);
        match layout.orientation {
            LayoutOrientation::Horizontal => {
                let x = baseline.0.x + (i as f64 * bar_w) + spacing / 2.0;
                let avail_up = baseline.0.y;
                let h = lvl * avail_up;
                let y = baseline.0.y - h;
                let w = bar_w - spacing;

                round_rect(ctx, x, y, w, h, 2.0, !is_stroke);
            }
            LayoutOrientation::Vertical => {
                let y = baseline.0.y + (i as f64 * bar_w) + spacing / 2.0;
                let avail_right = layout.space.x - baseline.0.x;
                let w = lvl * avail_right;
                let x = baseline.0.x;
                let h = bar_w - spacing;

                round_rect(ctx, x, y, w, h, 2.0, !is_stroke);
            }
        }
    }

    // Extend first/last bars under overlay margins so overlays have content underneath,
    // using CanvasTransform to convert overlay margins to world units.
    let left_world = tr.translate_x_margin_dev / tr.device_scale_x;
    let right_world = (tr.margin_right_css * tr.device_pixel_ratio) / tr.device_scale_x;
    let top_world = tr.translate_y_margin_dev / tr.device_scale_y;
    let bottom_world = (tr.margin_bottom_css * tr.device_pixel_ratio) / tr.device_scale_y;

    // Compute first/last levels matching the scaling above
    let first_lvl = if !excitements.is_empty() {
        let max_act = excitements
            .iter()
            .copied()
            .fold(0.0f32, |acc, v| acc.max(v.abs()));
        let scale2 = if max_act > 1.0 { 1.0 / max_act } else { 1.0 } as f64;
        (excitements[0] as f64 * scale2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let last_lvl = if !excitements.is_empty() {
        let max_act = excitements
            .iter()
            .copied()
            .fold(0.0f32, |acc, v| acc.max(v.abs()));
        let scale2 = if max_act > 1.0 { 1.0 / max_act } else { 1.0 } as f64;
        (excitements[excitements.len() - 1] as f64 * scale2).clamp(0.0, 1.0)
    } else {
        0.0
    };

    match layout.orientation {
        LayoutOrientation::Horizontal => {
            // First bar at the left overlay
            if left_world > 0.0 {
                let avail_up = baseline.0.y;
                let h = first_lvl * avail_up;
                let y = baseline.0.y - h;
                ctx.begin_path();
                ctx.move_to(0.0, baseline.0.y);
                ctx.line_to(0.0, y);
                ctx.line_to(-left_world, y);
                ctx.line_to(-left_world, baseline.0.y);
                ctx.close_path();
                ctx.fill();
                ctx.stroke();
            }
            // Last bar at the right overlay
            if right_world > 0.0 {
                let avail_up = baseline.0.y;
                let h = last_lvl * avail_up;
                let y = baseline.0.y - h;
                ctx.begin_path();
                ctx.move_to(layout.space.x, baseline.0.y);
                ctx.line_to(layout.space.x, y);
                ctx.line_to(layout.space.x + right_world, y);
                ctx.line_to(layout.space.x + right_world, baseline.0.y);
                ctx.close_path();
                ctx.fill();
                ctx.stroke();
            }
        }
        LayoutOrientation::Vertical => {
            // First bar at the top overlay
            if top_world > 0.0 {
                let avail_right = layout.space.x - baseline.0.x;
                let w = first_lvl * avail_right;
                let x = baseline.0.x + w;
                ctx.begin_path();
                ctx.move_to(baseline.0.x, 0.0);
                ctx.line_to(x, 0.0);
                ctx.line_to(x, -top_world);
                ctx.line_to(baseline.0.x, -top_world);
                ctx.close_path();
                ctx.fill();
                ctx.stroke();
            }
            // Last bar at the bottom overlay
            if bottom_world > 0.0 {
                let avail_right = layout.space.x - baseline.0.x;
                let w = last_lvl * avail_right;
                let x = baseline.0.x + w;
                ctx.begin_path();
                ctx.move_to(baseline.1.x, layout.space.y);
                ctx.line_to(x, layout.space.y);
                ctx.line_to(x, layout.space.y + bottom_world);
                ctx.line_to(baseline.1.x, layout.space.y + bottom_world);
                ctx.close_path();
                ctx.fill();
                ctx.stroke();
            }
        }
    }

    ctx.restore();
}

#[allow(clippy::too_many_arguments)]
fn draw_spectrum_area(
    ctx: &CanvasRenderingContext2d,
    cfg: &Config,
    tr: &super::CanvasTransform,
    layout: &TunerLayout,
    magnitudes: &[f32],
    frequencies: &[f32],
    alpha: f64,
    base_color: &str,
    secondary_color: &str,
) {
    if magnitudes.is_empty() || frequencies.is_empty() {
        return;
    }

    ctx.save();
    ctx.set_global_alpha(alpha);
    ctx.set_stroke_style_str(secondary_color);
    ctx.set_fill_style_str(base_color);
    ctx.set_line_width(1.0);

    // Begin path to draw polygon
    ctx.begin_path();

    let baseline = layout.line_position;

    // Start from appropriate edge based on orientation, at baseline
    match layout.orientation {
        LayoutOrientation::Horizontal => {
            if let Some((mag, freq)) = magnitudes.first().zip(frequencies.first()) {
                let p = cfg.frequency_magnitude_to_space(layout, *freq, *mag);
                ctx.move_to(p.x, baseline.0.y);
                ctx.line_to(p.x, p.y);
            } else {
                ctx.move_to(0.0, baseline.0.y);
            }
        }
        LayoutOrientation::Vertical => {
            if let Some((mag, freq)) = magnitudes.first().zip(frequencies.first()) {
                let p = cfg.frequency_magnitude_to_space(layout, *freq, *mag);
                ctx.move_to(baseline.0.x, p.y);
                ctx.line_to(p.x, p.y);
            } else {
                ctx.move_to(baseline.0.x, 0.0);
            }
        }
    }

    // Plot spectrum points
    for (i, &mag) in magnitudes.iter().enumerate() {
        if i < frequencies.len() {
            let p = cfg.frequency_magnitude_to_space(layout, frequencies[i], mag);
            ctx.line_to(p.x, p.y);
        }
    }

    // Close polygon back to baseline at opposite edge
    match layout.orientation {
        LayoutOrientation::Horizontal => {
            if let Some((mag, freq)) = magnitudes.last().zip(frequencies.last()) {
                let p_last = cfg.frequency_magnitude_to_space(layout, *freq, *mag);
                ctx.line_to(p_last.x, baseline.0.y);
            }
            if let Some((mag, freq)) = magnitudes.first().zip(frequencies.first()) {
                let p_first = cfg.frequency_magnitude_to_space(layout, *freq, *mag);
                ctx.line_to(p_first.x, baseline.0.y);
            }
        }
        LayoutOrientation::Vertical => {
            if let Some((mag, freq)) = magnitudes.last().zip(frequencies.last()) {
                let p_last = cfg.frequency_magnitude_to_space(layout, *freq, *mag);
                ctx.line_to(baseline.0.x, p_last.y);
            }
            if let Some((mag, freq)) = magnitudes.first().zip(frequencies.first()) {
                let p_first = cfg.frequency_magnitude_to_space(layout, *freq, *mag);
                ctx.line_to(baseline.0.x, p_first.y);
            }
        }
    }

    ctx.close_path();
    ctx.fill();
    ctx.stroke();

    // Extend constant-edge quads into overlay margins so bars have content underneath.
    // Use margin translations in device px converted to world units via per-axis device scales.
    let left_world = tr.translate_x_margin_dev / tr.device_scale_x;
    let right_world = (tr.margin_right_css * tr.device_pixel_ratio) / tr.device_scale_x;
    let top_world = tr.translate_y_margin_dev / tr.device_scale_y;
    let bottom_world = (tr.margin_bottom_css * tr.device_pixel_ratio) / tr.device_scale_y;

    match layout.orientation {
        LayoutOrientation::Horizontal => {
            // Left bar: extend first sample into negative X by left_world
            if left_world > 0.0 {
                if let Some((mag, freq)) = magnitudes.first().zip(frequencies.first()) {
                    let p_first = cfg.frequency_magnitude_to_space(layout, *freq, *mag);
                    ctx.begin_path();
                    ctx.move_to(0.0, baseline.0.y);
                    ctx.line_to(0.0, p_first.y);
                    ctx.line_to(-left_world, p_first.y);
                    ctx.line_to(-left_world, baseline.0.y);
                    ctx.close_path();
                    ctx.fill();
                    ctx.stroke();
                }
            }
            // Right bar: extend last sample beyond space.x by right_world
            if right_world > 0.0 {
                if let Some((mag, freq)) = magnitudes.last().zip(frequencies.last()) {
                    let p_last = cfg.frequency_magnitude_to_space(layout, *freq, *mag);
                    ctx.begin_path();
                    ctx.move_to(layout.space.x, baseline.0.y);
                    ctx.line_to(layout.space.x, p_last.y);
                    ctx.line_to(layout.space.x + right_world, p_last.y);
                    ctx.line_to(layout.space.x + right_world, baseline.0.y);
                    ctx.close_path();
                    ctx.fill();
                    ctx.stroke();
                }
            }
        }
        LayoutOrientation::Vertical => {
            // Top bar: extend first sample upward (negative Y) by top_world
            if top_world > 0.0 {
                if let Some((mag, freq)) = magnitudes.first().zip(frequencies.first()) {
                    let p_first = cfg.frequency_magnitude_to_space(layout, *freq, *mag);
                    ctx.begin_path();
                    ctx.move_to(baseline.0.x, 0.0);
                    ctx.line_to(p_first.x, 0.0);
                    ctx.line_to(p_first.x, -top_world);
                    ctx.line_to(baseline.0.x, -top_world);
                    ctx.close_path();
                    ctx.fill();
                    ctx.stroke();
                }
            }
            // Bottom bar: extend last sample beyond space.y by bottom_world
            if bottom_world > 0.0 {
                if let Some((mag, freq)) = magnitudes.last().zip(frequencies.last()) {
                    let p_last = cfg.frequency_magnitude_to_space(layout, *freq, *mag);
                    ctx.begin_path();
                    ctx.move_to(baseline.1.x, layout.space.y);
                    ctx.line_to(p_last.x, layout.space.y);
                    ctx.line_to(p_last.x, layout.space.y + bottom_world);
                    ctx.line_to(baseline.1.x, layout.space.y + bottom_world);
                    ctx.close_path();
                    ctx.fill();
                    ctx.stroke();
                }
            }
        }
    }

    ctx.restore();
}

fn round_rect(ctx: &CanvasRenderingContext2d, x: f64, y: f64, w: f64, h: f64, r: f64, fill: bool) {
    let rx = r.min(w / 2.0).max(0.0);
    let ry = r.min(h / 2.0).max(0.0);

    ctx.begin_path();
    ctx.move_to(x + rx, y);
    ctx.line_to(x + w - rx, y);
    ctx.quadratic_curve_to(x + w, y, x + w, y + ry);
    ctx.line_to(x + w, y + h - ry);
    ctx.quadratic_curve_to(x + w, y + h, x + w - rx, y + h);
    ctx.line_to(x + rx, y + h);
    ctx.quadratic_curve_to(x, y + h, x, y + h - ry);
    ctx.line_to(x, y + ry);
    ctx.quadratic_curve_to(x, y, x + rx, y);
    ctx.close_path();
    if fill {
        ctx.fill();
    } else {
        ctx.stroke();
    }
}
