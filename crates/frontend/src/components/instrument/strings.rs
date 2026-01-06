use common::instrument::GroupChannel;
use leptos::{html, prelude::*};
use leptos_use::use_device_pixel_ratio;
use tauri_use::{use_command, UseTauriWithReturn};
use web_sys::CanvasRenderingContext2d;

use crate::components::intro::consts::{INTRO_FLUTE_POS_X, INTRO_FLUTE_POS_Y, INTRO_FLUTE_ROT_DEG};
use crate::util::drawing::{get_2d_ctx, is_dark_mode, resolve_theme_color};
use crate::util::layout_context::{expect_layout_context, LayoutContextReturn};

#[derive(Debug, Clone, Default)]
struct VizEntry {
    left: Vec<Vec<f32>>,
    right: Vec<Vec<f32>>,
}

impl VizEntry {
    fn is_empty(&self) -> bool {
        self.left.is_empty() && self.right.is_empty()
    }
}

#[component]
pub fn InstrumentStrings() -> impl IntoView {
    const HISTORY_SIZE: usize = 3;

    let LayoutContextReturn {
        space,
        first_group_channel,
        left_string_position,
        right_string_position,
        complete_layout,
        ..
    } = expect_layout_context();

    let string_wave_amplitude = Memo::new(move |_| complete_layout().instrument_breadth * 0.55);

    let (viz_data, set_viz_data) =
        signal::<[VizEntry; HISTORY_SIZE]>(core::array::from_fn(|_| VizEntry::default()));

    // Instrument snoop batch stream (pull model).
    let UseTauriWithReturn {
        trigger: fetch_batch,
        data: batch_data,
        error: batch_error,
        ..
    } = use_command::<common::instrument::data::StringSnoopBatchPayload>(
        common::instrument::data::GET_ALL_STRING_SNOOPS,
    );

    // Log errors for the batch command
    Effect::new(move |_| {
        if let Some(err) = batch_error() {
            log::error!(
                "Error invoking {}: {err}",
                common::instrument::data::GET_ALL_STRING_SNOOPS
            );
        }
    });

    // Canvas reference and device pixel ratio
    let canvas_ref = NodeRef::<html::Canvas>::new();
    let pixel_ratio = use_device_pixel_ratio();

    let clear_cavas = move |ctx: Option<CanvasRenderingContext2d>| {
        if let Some(ctx) = ctx.or_else(|| canvas_ref.get().and_then(|canvas| get_2d_ctx(&canvas))) {
            let space = space();
            ctx.clear_rect(0.0, 0.0, space.x, space.y);
        }
    };

    // Setup canvas backing resolution and scaling whenever canvas mounts or layout/pixel ratio changes
    Effect::new(move |_| {
        if let Some(canvas) = canvas_ref.get() {
            let pr = pixel_ratio();
            let space = space();

            clear_cavas(None);

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

    let is_dark = is_dark_mode();
    let stroke_color = Memo::new(move |_| {
        let var = if is_dark() {
            "--color-red"
        } else {
            "--color-black"
        };
        resolve_theme_color(var).unwrap_or_else(|| {
            if is_dark() {
                "#e30022".into()
            } else {
                "#353839".into()
            }
        })
    });

    let last_ts = StoredValue::new(Option::<f64>::None);
    // Drive periodic fetch (20 FPS)
    let _raf = crate::util::raf_fn_fps::use_raf_fn_with_fps(
        move |_| {
            fetch_batch(Some(()));
            if let Some((batch, ctx)) = batch_data()
                .filter(|b| last_ts.get_value() != Some(b.t_unix_ms))
                .zip(
                    canvas_ref
                        .get_untracked()
                        .and_then(|canvas| get_2d_ctx(&canvas)),
                )
            {
                last_ts.set_value(Some(batch.t_unix_ms));
                clear_cavas(Some(ctx.clone()));

                let first_group_channel = first_group_channel.get_untracked();
                let (left, right) = batch.snoops.into_iter().fold(
                    (Vec::new(), Vec::new()),
                    |(mut left, mut right), snoop| {
                        match first_group_channel.nth_channel_from_first(snoop.group as usize) {
                            GroupChannel::Left => {
                                left.push(snoop.samples);
                            }
                            GroupChannel::Right => {
                                right.push(snoop.samples);
                            }
                        }

                        (left, right)
                    },
                );

                set_viz_data.update(|data| {
                    data.rotate_left(1);
                    data[HISTORY_SIZE - 1] = VizEntry { left, right };
                });
            }
        },
        24.0,
    );

    let slice_edge = StoredValue::new(0_usize);

    Effect::new(move |_| {
        let VizEntry { left, right } = viz_data().into_iter().fold(
            VizEntry::default(),
            |mut acc, VizEntry { left, right }| {
                if acc.is_empty() {
                    acc.left = left;
                    acc.right = right;
                } else {
                    acc.left.iter_mut().zip(left).for_each(|(a, b)| a.extend(b));
                    acc.right
                        .iter_mut()
                        .zip(right)
                        .for_each(|(a, b)| a.extend(b));
                }
                acc
            },
        );
        let slice_start = slice_edge.get_value();
        slice_edge.set_value((slice_start + 1) % left.first().map(|f| f.len()).unwrap_or(1));
        let stroke_color = stroke_color();
        let alpha_left = 2.0 / (left.len() as f64 + 1.0);
        let alpha_right = 2.0 / (right.len() as f64 + 1.0);
        if let Some(ctx) = canvas_ref.get().and_then(|canvas| get_2d_ctx(&canvas)) {
            let string_wave_amplitude = string_wave_amplitude();
            for mut samples in left {
                samples.rotate_left(slice_start);
                samples.reverse();
                draw_string_snoop_data(
                    &ctx,
                    left_string_position(),
                    &stroke_color,
                    string_wave_amplitude,
                    alpha_left,
                    &samples,
                );
            }
            for mut samples in right {
                samples.rotate_left(slice_start);
                samples.reverse();
                draw_string_snoop_data(
                    &ctx,
                    right_string_position(),
                    &stroke_color,
                    string_wave_amplitude,
                    alpha_right,
                    &samples,
                );
            }
        }
    });

    // Inner: rotation animation origin + variables
    let root_inner_style = move || {
        // Map artwork (intro composition) coordinates to current layout space
        let viewport_w = crate::components::intro::consts::INTRO_COMP_VIEWBOX_WIDTH;
        let viewport_h = crate::components::intro::consts::INTRO_COMP_VIEWBOX_HEIGHT;
        let layout = space();
        let scale_x = layout.x / viewport_w;
        let scale_y = layout.y / viewport_h;
        let viewport_scale = scale_x.min(scale_y);
        let viewport_scale_x = 1.0 - scale_x;
        let viewport_scale_y = 1.0 - scale_y;

        // Transform origin aligned with flute rotation pivot in screen/layout space, using bottom-right anchoring of the composition:
        // - Scale = min(space/viewBox)
        // - Top-left of the scaled composition in screen coords = (space.x - scaled_w, space.y - scaled_h)
        // - Pivot in screen coords = offset + pivot_in_viewBox * scale
        let scaled_w = viewport_w * viewport_scale;
        let scaled_h = viewport_h * viewport_scale;
        let offset_x = layout.x - scaled_w;
        let offset_y = layout.y - scaled_h;
        let origin_x = offset_x + INTRO_FLUTE_POS_X * viewport_scale;
        let origin_y = offset_y + INTRO_FLUTE_POS_Y * viewport_scale;

        // Start rotated to match flute angle, then normalize to 0deg
        let k1_rot = INTRO_FLUTE_ROT_DEG;

        // Compute initial translation so the strings group moves from the flute pivot to its final place.
        // Anchor = midpoint between channel midpoints (in screen/layout space).
        let (ls, le) = left_string_position();
        let (rs, re) = right_string_position();
        let left_mid_x = (ls.x + le.x) / 2.0;
        let left_mid_y = (ls.y + le.y) / 2.0;
        let right_mid_x = (rs.x + re.x) / 2.0;
        let right_mid_y = (rs.y + re.y) / 2.0;
        let anchor_x = (left_mid_x + right_mid_x) / 2.0;
        let anchor_y = (left_mid_y + right_mid_y) / 2.0;

        let k1_tx = origin_x - anchor_x;
        let k1_ty = origin_y - anchor_y;

        format!(
            concat!(
                "transform-box: view-box;",
                " transform-origin: {}px {}px;",
                " --inst-strings-k1-rot: {}deg;",
                " --inst-strings-k1-tx: {}px;",
                " --inst-strings-k1-ty: {}px;",
                " --inst-strings-k1-scale-x: {};",
                " --inst-strings-k1-scale-y: {};",
            ),
            origin_x, origin_y, k1_rot, k1_tx, k1_ty, viewport_scale_x, viewport_scale_y,
        )
    };

    view! {
        <canvas
            node_ref=canvas_ref
            // Width/height attributes are set via Effect to account for pixel ratio.
            // Initial attributes to avoid 0 size before first Effect runs.
            width=move || space().x as u32
            height=move || space().y as u32
            id="strings-root"
            class=super::instrument_animations::INSTRUMENT_STRINGS_ROOT_APPEAR
            style=root_inner_style
        ></canvas>
    }
}

fn draw_string_snoop_data(
    ctx: &CanvasRenderingContext2d,
    (start, end): common::Line,
    stroke_color: &str,
    amplitude: f64,
    alpha: f64,
    samples: &[f32],
) {
    ctx.save();

    ctx.set_stroke_style_str(stroke_color);
    ctx.set_global_alpha(alpha);
    ctx.set_line_width(1.75);
    ctx.set_filter("blur(1.5px)");

    ctx.begin_path();

    if samples.len() < 2 {
        ctx.move_to(start.x, start.y);
        ctx.line_to(end.x, end.y);
        ctx.stroke();
        ctx.restore();
        return;
    }

    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let len = (dx * dx + dy * dy).sqrt();
    if len <= f64::EPSILON {
        ctx.move_to(start.x, start.y);
        ctx.line_to(end.x, end.y);
        ctx.stroke();
        ctx.restore();
        return;
    }

    // Unit normal to the line (perpendicular)
    let nx = -dy / len;
    let ny = dx / len;

    let points = samples.len();

    for (i, &src) in samples.iter().enumerate() {
        let t = i as f64 / (points - 1) as f64;
        let px = start.x + dx * t;
        let py = start.y + dy * t;

        // Match existing convention: invert sample for displacement direction.
        let off = -src as f64 * amplitude;
        let x = px + off * nx;
        let y = py + off * ny;

        if i == 0 {
            ctx.move_to(x, y);
        } else {
            ctx.line_to(x, y);
        }
    }

    ctx.stroke();
    ctx.restore();
}
