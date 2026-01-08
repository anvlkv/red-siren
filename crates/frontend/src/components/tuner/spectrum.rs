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

    // scale is now provided by the parent Tuner via props

    // Baseline from layout (no fallback)
    let baseline = Memo::new(move |_| layout.with(|l| l.as_ref().map(|lay| lay.line_position)));

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
        if let Some(lay) = layout() {
            if let Some(canvas) = canvas_ref.get() {
                let _space = lay.space;

                // Set backing resolution in device pixels (provided by parent)
                let tr = canvas_transform();
                canvas.set_width(tr.backing_width_dev);
                canvas.set_height(tr.backing_height_dev);

                // Apply uniform scale with letterboxing/pillarboxing (provided transform)
                if let Some(ctx) = get_2d_ctx(&canvas) {
                    _ = ctx.reset_transform().ok();
                    _ = ctx.translate(tr.translate_x_dev, tr.translate_y_dev).ok();
                    _ = ctx.scale(tr.device_scale, tr.device_scale).ok();
                }
            }
        }
    });

    let UseTauriWithReturn {
        data: spectrum_data,
        error: spectrum_error,
        trigger: poll_spectrum,
    } = use_command::<SpectrumData>(common::commands::tuner::SPECTRUM_DATA);

    _ = use_raf_fn_with_fps(
        move |_| {
            poll_spectrum(Some(()));
            if let Some(spectrum) = spectrum_data() {
                let Some(lay) = layout() else { return };

                if let Some(ctx) = canvas_ref.get().and_then(|canvas| get_2d_ctx(&canvas)) {
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
                    if let Some(bl) = baseline.get() {
                        draw_baseline(&ctx, bl, &base_color);
                    }

                    // Draw max-hold sensor excitement outline bars (lower opacity strokes)
                    if let Some(bl) = baseline.get() {
                        draw_excitement_bars(
                            &ctx,
                            &lay,
                            bl,
                            &spectrum.max_excitements,
                            &base_color,
                            &secondary_color,
                            0.40,
                            true,
                        );
                    }

                    // Draw sensor excitement bars (filled, higher opacity)
                    if let Some(bl) = baseline.get() {
                        draw_excitement_bars(
                            &ctx,
                            &lay,
                            bl,
                            &spectrum.sensor_excitements,
                            &base_color,
                            &secondary_color,
                            0.30,
                            false,
                        );
                    }

                    // Draw spectrum layers
                    let cfg = Config {
                        sensor_data: vec![],
                        sample_rate: spectrum.sample_rate,
                        fft_size: spectrum.fft_size,
                    };

                    if !spectrum.max_magnitudes.is_empty() && !spectrum.frequencies.is_empty() {
                        draw_spectrum_area(
                            &ctx,
                            &cfg,
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
                            &lay,
                            &spectrum.current_magnitudes,
                            &spectrum.frequencies,
                            0.60, // alpha to match "fill-gray/40 ... stroke-.../60"
                            &base_color,
                            &secondary_color,
                        );
                    }
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
    });

    view! {
        <canvas
            node_ref=canvas_ref
            style:touch-action="none"
            class="absolute inset-0 w-full h-full"
        ></canvas>
    }
}

fn draw_baseline(ctx: &CanvasRenderingContext2d, baseline: common::Line, base_color: &str) {
    ctx.save();
    ctx.set_stroke_style_str(base_color);
    ctx.set_line_width(1.0);

    ctx.begin_path();
    ctx.move_to(baseline.0.x, baseline.0.y);
    ctx.line_to(baseline.1.x, baseline.1.y);
    ctx.stroke();
    ctx.restore();
}

#[allow(clippy::too_many_arguments)]
fn draw_excitement_bars(
    ctx: &CanvasRenderingContext2d,
    layout: &TunerLayout,
    baseline: common::Line,
    excitements: &[f32],
    base_color: &str,
    secondary_color: &str,
    alpha: f64,
    is_stroke: bool,
) {
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

    ctx.restore();
}

#[allow(clippy::too_many_arguments)]
fn draw_spectrum_area(
    ctx: &CanvasRenderingContext2d,
    cfg: &Config,
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
            ctx.move_to(0.0, baseline.0.y);
            if let Some((mag, freq)) = magnitudes.first().zip(frequencies.first()) {
                let p = cfg.frequency_magnitude_to_space(layout, *freq, *mag);
                ctx.line_to(0.0, p.y);
            }
        }
        LayoutOrientation::Vertical => {
            ctx.move_to(baseline.0.x, 0.0);
            if let Some((mag, freq)) = magnitudes.first().zip(frequencies.first()) {
                let p = cfg.frequency_magnitude_to_space(layout, *freq, *mag);
                ctx.line_to(p.x, 0.0);
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
                let p = cfg.frequency_magnitude_to_space(layout, *freq, *mag);
                ctx.line_to(layout.space.x, p.y);
            }
            ctx.line_to(layout.space.x, baseline.0.y);
            ctx.line_to(0.0, baseline.0.y);
        }
        LayoutOrientation::Vertical => {
            if let Some((mag, freq)) = magnitudes.last().zip(frequencies.last()) {
                let p = cfg.frequency_magnitude_to_space(layout, *freq, *mag);
                ctx.line_to(p.x, layout.space.y);
            }
            ctx.line_to(baseline.0.x, layout.space.y);
            ctx.line_to(baseline.0.x, 0.0);
        }
    }

    ctx.close_path();
    ctx.fill();
    ctx.stroke();
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
