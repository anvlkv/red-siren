use leptos::{html, prelude::*};
use leptos_use::use_device_pixel_ratio;
use web_sys::CanvasRenderingContext2d;

use common::orientation::LayoutOrientation;
use common::tuner::{Config, Layout as TunerLayout, SpectrumData};

use crate::util::drawing::{get_2d_ctx, is_dark_mode, resolve_theme_color};

/// Spectrum visualizer component for displaying FFT analysis results using HTML Canvas
#[component]
pub fn SpectrumVisualizer(
    /// Spectrum data signal
    #[prop(into)]
    spectrum: Signal<Option<SpectrumData>>,

    /// Tuner layout (space, orientation, baseline, sensors count)
    #[prop(into)]
    layout: Signal<Option<TunerLayout>>,
) -> impl IntoView {
    // Canvas reference and device pixel ratio
    let canvas_ref = NodeRef::<html::Canvas>::new();
    let pixel_ratio = use_device_pixel_ratio();

    // Baseline from layout (no fallback)
    let baseline = Memo::new(move |_| layout.with(|l| l.as_ref().map(|lay| lay.line_position)));

    // Prepare resolved theme colors once per draw
    let stroke_fill_colors = Memo::new(move |_| {
        let is_dark = is_dark_mode();
        let base_var = if is_dark {
            "--color-red"
        } else {
            "--color-black"
        };
        let base_color = resolve_theme_color(base_var).unwrap_or_else(|| {
            if is_dark {
                "#e30022".into()
            } else {
                "#353839".into()
            }
        });

        // Higher alpha layers use same hue, different globalAlpha
        // Provide tuple (base_color, is_dark)
        (base_color, is_dark)
    });

    // Setup canvas backing resolution and scaling whenever canvas mounts or layout/pixel ratio changes
    Effect::new(move |_| {
        if let Some(canvas) = canvas_ref.get() {
            if let Some(lay) = layout() {
                let pr = pixel_ratio();
                let space = lay.space;

                // Set CSS size in CSS pixels
                let _ = canvas.set_attribute(
                    "style",
                    &format!("width: {}px; height: {}px;", space.x, space.y),
                );

                // Set backing resolution in device pixels
                let logical_w = space.x.max(0.0);
                let logical_h = space.y.max(0.0);
                let backing_w = (logical_w * pr).round().clamp(1.0, f64::MAX) as u32;
                let backing_h = (logical_h * pr).round().clamp(1.0, f64::MAX) as u32;
                canvas.set_width(backing_w);
                canvas.set_height(backing_h);

                // Acquire 2d context and scale so drawing uses logical CSS pixels
                if let Some(ctx) = get_2d_ctx(&canvas) {
                    _ = ctx.reset_transform().ok();
                    _ = ctx.scale(pr, pr).ok();
                }
            }
        }
    });

    // Draw baseline, spectrum layers, and excitement bars when spectrum or layout changes
    Effect::new(move |_| {
        let Some(lay) = layout() else { return };
        let Some(s) = spectrum() else { return };

        let space = lay.space;

        if let Some(ctx) = canvas_ref.get().and_then(|canvas| get_2d_ctx(&canvas)) {
            // Clear canvas
            ctx.clear_rect(0.0, 0.0, space.x, space.y);

            // Resolve base color and theme
            let (base_color, is_dark) = stroke_fill_colors.get();

            // Configure blend to match original SVG classes intent
            // We mimic mix-blend-plus-darker/lighter with globalCompositeOperation hints.
            // Fallback to "source-over" if unsupported.
            _ = ctx.set_global_composite_operation("source-over");

            // Draw baseline reference line
            if let Some(bl) = baseline.get() {
                draw_baseline(&ctx, bl, &base_color, is_dark);
            }

            // Draw max-hold sensor excitement outline bars (lower opacity strokes)
            if let Some(bl) = baseline.get() {
                draw_excitement_bars_outline(
                    &ctx,
                    &lay,
                    bl,
                    &s.max_excitements,
                    &base_color,
                    is_dark,
                );
            }

            // Draw sensor excitement bars (filled, higher opacity)
            if let Some(bl) = baseline.get() {
                draw_excitement_bars_fill(
                    &ctx,
                    &lay,
                    bl,
                    &s.sensor_excitements,
                    &base_color,
                    is_dark,
                );
            }

            // Draw spectrum layers
            let cfg = Config {
                sensor_data: vec![],
                sample_rate: s.sample_rate,
            };

            if !s.max_magnitudes.is_empty() && !s.frequencies.is_empty() {
                draw_spectrum_area(
                    &ctx,
                    &cfg,
                    &lay,
                    &s.max_magnitudes,
                    &s.frequencies,
                    0.30, // alpha to match "fill-gray/30 ... stroke-.../50"
                    &base_color,
                    is_dark,
                );
            }

            if !s.current_magnitudes.is_empty() && !s.frequencies.is_empty() {
                draw_spectrum_area(
                    &ctx,
                    &cfg,
                    &lay,
                    &s.current_magnitudes,
                    &s.frequencies,
                    0.40, // alpha to match "fill-gray/40 ... stroke-.../60"
                    &base_color,
                    is_dark,
                );
            }
        }
    });

    view! {
        <canvas
            node_ref=canvas_ref
            // Initial attributes to avoid 0 size before first Effect runs.
            width=800
            height=600
            class="absolute inset-0 w-full h-full"
        ></canvas>
    }
}

fn draw_baseline(
    ctx: &CanvasRenderingContext2d,
    baseline: common::Line,
    base_color: &str,
    is_dark: bool,
) {
    let stroke = themed_stroke_color(base_color, is_dark, 0.20);
    ctx.save();
    ctx.set_stroke_style_str(&stroke);
    ctx.set_line_width(1.0);

    ctx.begin_path();
    ctx.move_to(baseline.0.x, baseline.0.y);
    ctx.line_to(baseline.1.x, baseline.1.y);
    ctx.stroke();
    ctx.restore();
}

fn draw_excitement_bars_outline(
    ctx: &CanvasRenderingContext2d,
    layout: &TunerLayout,
    baseline: common::Line,
    excitements: &[f32],
    base_color: &str,
    is_dark: bool,
) {
    let stroke = themed_stroke_color(base_color, is_dark, 0.40);
    ctx.save();
    ctx.set_stroke_style_str(&stroke);
    ctx.set_line_width(0.5);

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

                ctx.stroke_rect(x, y, w, h);
            }
            LayoutOrientation::Vertical => {
                let y = baseline.0.y + (i as f64 * bar_w) + spacing / 2.0;
                let avail_right = layout.space.x - baseline.0.x;
                let w = lvl * avail_right;
                let x = baseline.0.x;
                let h = bar_w - spacing;

                ctx.stroke_rect(x, y, w, h);
            }
        }
    }

    ctx.restore();
}

fn draw_excitement_bars_fill(
    ctx: &CanvasRenderingContext2d,
    layout: &TunerLayout,
    baseline: common::Line,
    excitements: &[f32],
    base_color: &str,
    is_dark: bool,
) {
    let fill = themed_fill_color(base_color, is_dark, 0.20);
    ctx.save();
    ctx.set_fill_style_str(&fill);

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

                round_rect_fill(ctx, x, y, w, h, 2.0);
            }
            LayoutOrientation::Vertical => {
                let y = baseline.0.y + (i as f64 * bar_w) + spacing / 2.0;
                let avail_right = layout.space.x - baseline.0.x;
                let w = lvl * avail_right;
                let x = baseline.0.x;
                let h = bar_w - spacing;

                round_rect_fill(ctx, x, y, w, h, 2.0);
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
    is_dark: bool,
) {
    if magnitudes.is_empty() || frequencies.is_empty() {
        return;
    }

    // Stroke and fill colors with themed alpha
    let stroke = themed_stroke_color(base_color, is_dark, alpha.max(0.3)); // mimic stroke-gray/50..60
    let fill = themed_fill_color(base_color, is_dark, alpha);

    ctx.save();
    ctx.set_stroke_style_str(&stroke);
    ctx.set_fill_style_str(&fill);
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

fn themed_stroke_color(base_color: &str, is_dark: bool, alpha: f64) -> String {
    // Use same color with alpha modulation for strokes; in dark mode keep a bit brighter
    let a = alpha.clamp(0.0, 1.0);
    if base_color.starts_with('#') {
        // Convert hex to rgba approximately; fallback to base color if parsing fails.
        if let Some((r, g, b)) = parse_hex_rgb(base_color) {
            format!("rgba({},{},{},{})", r, g, b, a)
        } else {
            base_color.to_string()
        }
    } else {
        // If base color is not hex (e.g., var resolved to rgb), still apply alpha with globalAlpha.
        // We return color as-is and rely on globalAlpha in callers when needed.
        // Here, we embed alpha via rgba for safer rendering.
        if let Some((r, g, b)) = parse_rgb(base_color) {
            format!("rgba({},{},{},{})", r, g, b, a)
        } else {
            // Generic fallback
            if is_dark {
                format!("rgba(227,0,34,{})", a) // cinnabar-ish
            } else {
                format!("rgba(53,56,57,{})", a) // black-ish
            }
        }
    }
}

fn themed_fill_color(base_color: &str, is_dark: bool, alpha: f64) -> String {
    // Similar to stroke but slightly different alpha expectations
    themed_stroke_color(base_color, is_dark, alpha)
}

fn round_rect_fill(ctx: &CanvasRenderingContext2d, x: f64, y: f64, w: f64, h: f64, r: f64) {
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
    ctx.fill();
}

fn parse_hex_rgb(hex: &str) -> Option<(u8, u8, u8)> {
    let s = hex.trim_start_matches('#');
    let (r, g, b) = match s.len() {
        3 => {
            let r = u8::from_str_radix(&s[0..1].repeat(2), 16).ok()?;
            let g = u8::from_str_radix(&s[1..2].repeat(2), 16).ok()?;
            let b = u8::from_str_radix(&s[2..3].repeat(2), 16).ok()?;
            (r, g, b)
        }
        6 => {
            let r = u8::from_str_radix(&s[0..2], 16).ok()?;
            let g = u8::from_str_radix(&s[2..4], 16).ok()?;
            let b = u8::from_str_radix(&s[4..6], 16).ok()?;
            (r, g, b)
        }
        _ => return None,
    };
    Some((r, g, b))
}

fn parse_rgb(s: &str) -> Option<(u8, u8, u8)> {
    // crude parser for "rgb(r, g, b)" or "rgba(r, g, b, a)"
    let lower = s.trim().to_lowercase();
    let inner = if lower.starts_with("rgb(") && lower.ends_with(')') {
        &lower[4..lower.len() - 1]
    } else if lower.starts_with("rgba(") && lower.ends_with(')') {
        &lower[5..lower.len() - 1]
    } else {
        return None;
    };

    let parts: Vec<&str> = inner.split(',').map(|p| p.trim()).collect();
    if parts.len() < 3 {
        return None;
    }
    let r = parts[0].parse::<f64>().ok()? as u8;
    let g = parts[1].parse::<f64>().ok()? as u8;
    let b = parts[2].parse::<f64>().ok()? as u8;
    Some((r, g, b))
}
