use common::safe_area::SafeArea;
use leptos::{html, prelude::*};
use leptos_use::use_device_pixel_ratio;
use tauri_use::{use_command, UseTauriWithReturn};
use web_sys::CanvasRenderingContext2d;

use crate::{
    components::instrument::util::{get_2d_ctx, is_dark_mode, resolve_theme_color},
    util::{
        layout_context::{expect_layout_contex, LayoutContextReturn},
        raf_fn_fps::use_raf_fn_with_fps,
    },
};

#[component]
pub fn SpectrumViz() -> impl IntoView {
    // Layout and tauri invoke
    let LayoutContextReturn {
        space,
        orientation,
        safe_area_padding,
        ..
    } = expect_layout_contex();

    let UseTauriWithReturn {
        trigger: fetch_spectrum,
        data: spectrum_data,
        error: spectrum_error,
        ..
    } = use_command::<Option<common::instrument::commands::SpectrumPayload>>(
        common::instrument::commands::SNAPSHOT_PROCESSED_OUTPUT_SPECTRUM,
    );

    // Canvas reference and device pixel ratio
    let canvas_ref = NodeRef::<html::Canvas>::new();
    let pixel_ratio = use_device_pixel_ratio();

    // Compute cell size based on max bin count across history
    let (cell_size, set_cell_size) = signal(14_f64);
    let (current_row, set_current_row) = signal(0_usize);

    let clear_cavas = move || {
        if let Some(ctx) = canvas_ref.get().and_then(|canvas| get_2d_ctx(&canvas)) {
            let space = space();
            ctx.clear_rect(0.0, 0.0, space.x, space.y);
        }
    };

    // How many rows fit in the available space
    let history_length = Memo::new(move |prev: Option<&usize>| {
        let space = space();
        let cell_size = cell_size();
        let new_len = match orientation() {
            common::orientation::LayoutOrientation::Vertical => {
                (space.y / cell_size).ceil() as usize
            }
            common::orientation::LayoutOrientation::Horizontal => {
                (space.x / cell_size).ceil() as usize
            }
        };

        if prev.is_some_and(|&p| p != new_len) {
            clear_cavas();
        }

        new_len
    });

    // Setup canvas backing resolution and scaling whenever canvas mounts or layout/pixel ratio changes
    Effect::new(move |_| {
        if let Some(canvas) = canvas_ref.get() {
            let pr = pixel_ratio();
            let space = space();

            clear_cavas();

            // Set CSS size in CSS pixels
            canvas
                .set_attribute(
                    "style",
                    &format!("width: {}px; height: {}px;", space.x, space.y),
                )
                .ok();

            // Set backing resolution in device pixels
            let logical_w = space.x.max(0.0);
            let logical_h = space.y.max(0.0);
            let backing_w = (logical_w * pr).round().clamp(1.0, f64::MAX) as u32;
            let backing_h = (logical_h * pr).round().clamp(1.0, f64::MAX) as u32;
            canvas.set_width(backing_w);
            canvas.set_height(backing_h);

            // Acquire 2d context and scale so drawing uses logical CSS pixels
            if let Some(ctx) = get_2d_ctx(&canvas) {
                // Reset transform then scale by pixel ratio
                // Note: set_transform requires a DOMMatrix; here we use reset_transform for clarity
                _ = ctx.reset_transform().ok();
                _ = ctx.scale(pr, pr).ok();
            }
        }
    });

    let last_ts = StoredValue::new(Option::<f64>::None);

    // Fetch spectrum and draw at ~20 FPS
    let _raf = use_raf_fn_with_fps(
        move |_| {
            // Fetch latest spectrum snapshot
            fetch_spectrum(Some(()));
            if let Some(common::instrument::commands::SpectrumPayload { data, t_unix_ms }) =
                spectrum_data()
                    .flatten()
                    .filter(|d| last_ts.get_value() != Some(d.t_unix_ms))
            {
                last_ts.set_value(Some(t_unix_ms));

                let cell_size = cell_size.get_untracked();
                let space = space.get_untracked();
                let safe_area_padding = safe_area_padding.get_untracked();
                let orientation = orientation.get_untracked();
                let safe_x = space.x - safe_area_padding.left - safe_area_padding.right;
                let safe_y = space.y - safe_area_padding.top - safe_area_padding.bottom;
                let safe_space = mint::Vector2 {
                    x: safe_x,
                    y: safe_y,
                };

                let new_cell_size = comp_cell_size(safe_space, orientation, data.len());

                if (new_cell_size - cell_size).abs() > f64::EPSILON {
                    set_cell_size(new_cell_size);
                }

                let history_length = history_length.get_untracked();
                let current_row = current_row.get_untracked();

                // Draw current visualization
                if let Some(ctx) = canvas_ref.get().and_then(|canvas| get_2d_ctx(&canvas)) {
                    let (cell_w, cell_h) = {
                        let aux_size = comp_aux_size(safe_space, orientation, history_length);

                        match orientation {
                            common::orientation::LayoutOrientation::Vertical => {
                                (new_cell_size, aux_size)
                            }
                            common::orientation::LayoutOrientation::Horizontal => {
                                (aux_size, new_cell_size)
                            }
                        }
                    };

                    draw_spectrum(
                        &ctx,
                        data,
                        current_row,
                        orientation,
                        safe_x,
                        safe_y,
                        safe_area_padding,
                        cell_w,
                        cell_h,
                    );
                    set_current_row.update(|current| {
                        *current = current.checked_sub(1).unwrap_or(history_length - 1)
                    });
                }
            }
        },
        20.0,
    );

    // Error logging for invoke failures
    Effect::new(move |_| {
        if let Some(err) = spectrum_error() {
            log::error!(
                "Error invoking {}: {err}",
                common::instrument::commands::SNAPSHOT_PROCESSED_OUTPUT_SPECTRUM
            );
        }
    });

    // Canvas view replaces the prior SVG
    view! {
        <canvas
            node_ref=canvas_ref
            // Width/height attributes are set via Effect to account for pixel ratio.
            // Initial attributes to avoid 0 size before first Effect runs.
            width=800
            height=600
        ></canvas>
    }
}

const MIN_CELL_SIZE: f64 = 0.1;

fn comp_aux_size(
    space: mint::Vector2<f64>,
    orientation: common::orientation::LayoutOrientation,
    num_entries: usize,
) -> f64 {
    let aux = match orientation {
        common::orientation::LayoutOrientation::Vertical => space.y,
        common::orientation::LayoutOrientation::Horizontal => space.x,
    } / num_entries as f64;

    if aux.is_normal() && aux >= MIN_CELL_SIZE {
        aux
    } else {
        MIN_CELL_SIZE
    }
}

fn comp_cell_size(
    space: mint::Vector2<f64>,
    orientation: common::orientation::LayoutOrientation,
    num_entries: usize,
) -> f64 {
    let res = match orientation {
        common::orientation::LayoutOrientation::Vertical => space.x,
        common::orientation::LayoutOrientation::Horizontal => space.y,
    } / (num_entries as f64 * 2.0);

    if res.is_normal() && res >= MIN_CELL_SIZE {
        res
    } else {
        MIN_CELL_SIZE
    }
}

// Drawing function replacing SpectrumDataRow component rendering
#[allow(clippy::too_many_arguments)]
fn draw_spectrum(
    ctx: &CanvasRenderingContext2d,
    row: Vec<(f32, f32)>,
    row_index: usize,
    orientation: common::orientation::LayoutOrientation,
    width: f64,
    height: f64,
    safe_area_padding: SafeArea,
    cell_width: f64,
    cell_height: f64,
) {
    const BASE_ALPHA: f64 = 0.8;
    // Resolve theme color based on dark mode; fallback to cinnabar
    let is_dark = is_dark_mode();
    let var = if is_dark {
        "--color-cinnabar"
    } else {
        "--color-gray"
    };
    let fill_color = resolve_theme_color(var).unwrap_or_else(|| {
        if is_dark {
            "#e44d2e".into()
        } else {
            "#36454f".into()
        }
    });
    ctx.set_fill_style_str(&fill_color);

    // Precompute incremental positions and base split
    let cell_increment = match orientation {
        common::orientation::LayoutOrientation::Vertical => {
            cell_width.min(width / row.len() as f64)
        }
        common::orientation::LayoutOrientation::Horizontal => {
            cell_height.min(height / row.len() as f64)
        }
    };
    // Center the right channel base within the safe area.
    // `width`/`height` here already represent safe area dimensions, so do not subtract padding again.
    let right_base = match orientation {
        common::orientation::LayoutOrientation::Vertical => width / 2.0,
        common::orientation::LayoutOrientation::Horizontal => height / 2.0,
    };

    // Translate the context to the row position
    let (tx, ty) = match orientation {
        common::orientation::LayoutOrientation::Vertical => {
            (safe_area_padding.left, row_index as f64 * cell_height)
        }
        common::orientation::LayoutOrientation::Horizontal => {
            (row_index as f64 * cell_width, safe_area_padding.top)
        }
    };
    ctx.clear_rect(
        tx,
        ty,
        match orientation {
            common::orientation::LayoutOrientation::Vertical => width,
            common::orientation::LayoutOrientation::Horizontal => cell_width,
        },
        match orientation {
            common::orientation::LayoutOrientation::Vertical => cell_height,
            common::orientation::LayoutOrientation::Horizontal => height,
        },
    );
    ctx.save();
    ctx.translate(tx, ty).ok();

    // Draw each pair as two rectangles using global alpha scaled 0..1
    for (i, &(l, r)) in row.iter().enumerate() {
        let i = i as f64;
        match orientation {
            common::orientation::LayoutOrientation::Vertical => {
                // Left channel
                ctx.set_global_alpha(l as f64 * BASE_ALPHA);
                ctx.fill_rect(cell_increment * i, 0.0, cell_width, cell_height);
                // Right channel
                ctx.set_global_alpha(r as f64 * BASE_ALPHA);
                ctx.fill_rect(
                    cell_increment * i + right_base,
                    0.0,
                    cell_width,
                    cell_height,
                );
            }
            common::orientation::LayoutOrientation::Horizontal => {
                // Left channel
                ctx.set_global_alpha(l as f64 * BASE_ALPHA);
                ctx.fill_rect(0.0, cell_increment * i, cell_width, cell_height);
                // Right channel
                ctx.set_global_alpha(r as f64 * BASE_ALPHA);
                ctx.fill_rect(
                    0.0,
                    cell_increment * i + right_base,
                    cell_width,
                    cell_height,
                );
            }
        }
    }

    ctx.restore();

    // Restore opaque drawing
    ctx.set_global_alpha(1.0);
}
