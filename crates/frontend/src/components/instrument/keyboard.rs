use common::safe_area::SafeArea;
use leptos::prelude::*;

use crate::{
    components::{Button, UiSize},
    util::layout_context::{expect_layout_contex, LayoutContextReturn},
};

use super::{expect_instrument_context, instrument_animations};
use leptos::html::Div;
use leptos_use::use_element_bounding;
use tauri_use::{use_command, UseTauriWithReturn};

const BAND_STROKE_WIDTH: f32 = 4.0;

#[component]
pub fn Keyboard() -> impl IntoView {
    let LayoutContextReturn {
        orientation,
        space,
        safe_area_padding,
        num_groups,
        key_band_length,
        groups_gap,
        key_bands_gap,
        key_band_breadth,
        key_radius,
        key_pad_main,
        ..
    } = expect_layout_contex();

    // Activation snoop batch stream (pull model).
    let UseTauriWithReturn {
        trigger: fetch_batch,
        data: batch_data,
        error: batch_error,
        ..
    } = use_command::<common::instrument::data::ActivationSnoopBatchPayload>(
        common::instrument::data::GET_ALL_ACTIVATION_SNOOPS,
    );

    // Log errors for the batch command
    Effect::new(move |_| {
        if let Some(err) = batch_error() {
            log::error!(
                "Error invoking {}: {err}",
                common::instrument::data::GET_ALL_ACTIVATION_SNOOPS
            );
        }
    });

    // Drive periodic fetch (20 FPS)
    let _raf = crate::util::raf_fn_fps::use_raf_fn_with_fps(
        move |_| {
            fetch_batch(Some(()));
        },
        20.0,
    );

    let main_container_axis_style = Memo::new(move |_| {
        let orientation = orientation();
        let space = space();
        let safe_area_padding = safe_area_padding();
        let num_groups = num_groups();
        let key_band_length = key_band_length();
        let groups_gap = groups_gap();
        let key_bands_gap = key_bands_gap();
        let key_band_breadth = key_band_breadth();
        let key_radius = key_radius();
        let pad_main = key_pad_main();

        // Map the orientation-dependent safe area format into CSS box-model (top,right,bottom,left)
        let SafeArea {
            top: safe_top,
            right: safe_right,
            bottom: safe_bottom,
            left: safe_left,
        } = safe_area_padding;

        let mut defs = match orientation {
            common::orientation::LayoutOrientation::Vertical => format!(
                r#"
                --keyboard-rows: repeat({rows}, minmax(0, 1fr));
                --keyboard-cols: repeat({cols}, minmax(0, 1fr));
                --keyboard-row-gap: {row_gap}px;
                --keyboard-col-gap: {col_gap}px;
                --keyboard-pad-left: {pad_left}px;
                --keyboard-pad-right: {pad_right}px;
                --keyboard-pad-top: {pad_top}px;
                --keyboard-pad-bottom: {pad_bottom}px;
                "#,
                rows = num_groups,
                cols = 1,
                row_gap = groups_gap,
                col_gap = 0,
                pad_left = safe_left,
                pad_right = safe_right,
                pad_top = pad_main.max(safe_top),
                pad_bottom = pad_main.max(safe_bottom)
            ),
            common::orientation::LayoutOrientation::Horizontal => format!(
                r#"
                --keyboard-rows: repeat({rows}, minmax(0, 1fr));
                --keyboard-cols: repeat({cols}, minmax(0, 1fr));
                --keyboard-row-gap: {row_gap}px;
                --keyboard-col-gap: {col_gap}px;
                --keyboard-pad-left: {pad_left}px;
                --keyboard-pad-right: {pad_right}px;
                --keyboard-pad-top: {pad_top}px;
                --keyboard-pad-bottom: {pad_bottom}px;
                "#,
                rows = 1,
                cols = num_groups,
                row_gap = 0,
                col_gap = groups_gap,
                pad_left = pad_main.max(safe_left),
                pad_right = pad_main.max(safe_right),
                pad_top = safe_top,
                pad_bottom = safe_bottom
            ),
        };

        defs.push_str(&format!("--keyboard-keys-gap: {}px;", key_bands_gap));

        // Compute padding so the key is exactly centered inside the band.
        let inner_band = key_band_breadth;
        let desired_diameter = key_radius * 2.0;

        defs.push_str(&format!(
            r#"
            --keyboard-band-breadth: {breadth}px;
            --keyboard-band-length: {length}px;
            --keyboard-key-diameter: {diameter}px;
            --keyboard-key-padding: {key_pad}px;
            --keyboard-band-stroke-width: {band_stroke}px;

            /* Explicit sizing + padding for outer container */
            width: {space_w}px;
            height: {space_h}px;
            padding: {safe_top}px {safe_right}px {safe_bottom}px {safe_left}px;
            "#,
            breadth = key_band_breadth,
            length = key_band_length,
            diameter = { desired_diameter.min(inner_band.max(0.0)) },
            key_pad = {
                // Clamp diameter to the available inner breadth (never negative)
                let dia = desired_diameter.min(inner_band.max(0.0));
                // Evenly distribute the remaining space on both sides to keep the key perfectly centered
                ((inner_band - dia) / 2.0).max(0.0)
            },
            space_w = space.x,
            space_h = space.y,
            safe_top = safe_top,
            safe_right = safe_right,
            safe_bottom = safe_bottom,
            safe_left = safe_left,
            band_stroke = BAND_STROKE_WIDTH,
        ));

        defs
    });

    view! {
        <div style=main_container_axis_style class="flex items-center justify-center">
            <div class="grid items-center justify-center grid-rows-(--keyboard-rows) grid-cols-(--keyboard-cols) gap-y-(--keyboard-row-gap) gap-x-(--keyboard-col-gap) p-t-(length:--keyboard-pad-top) p-b-(length:--keyboard-pad-bottom) p-l-(length:--keyboard-pad-left) p-r-(length:--keyboard-pad-right) w-full h-full">
                {move || {
                    let num_groups = num_groups();
                    (0..num_groups as usize)
                        .rev()
                        .map(|g| {
                            view! { <Group g batch_data=batch_data /> }
                        })
                        .collect_view()
                }}
            </div>
        </div>
    }
}

#[component]
fn Group(
    g: usize,
    #[prop(into)] batch_data: Signal<Option<common::instrument::data::ActivationSnoopBatchPayload>>,
) -> impl IntoView {
    let LayoutContextReturn {
        orientation,
        num_keys_per_group,
        first_group_channel,
        key_band_length,
        key_band_breadth,
        ..
    } = expect_layout_contex();

    let class = move || {
        format!(
            "flex {} gap-(--keyboard-keys-gap) justify-center items-center",
            match orientation() {
                common::orientation::LayoutOrientation::Vertical => "flex-col w-full",
                common::orientation::LayoutOrientation::Horizontal => "flex-row h-full",
            }
        )
    };

    let group_band_dims_style = move || {
        let len = key_band_length();
        let br = key_band_breadth();
        let square_size = br.min(len);
        let (final_w, final_h) = match orientation() {
            common::orientation::LayoutOrientation::Vertical => (len, br),
            common::orientation::LayoutOrientation::Horizontal => (br, len),
        };
        format!(
            "--inst-band-k1-width: {}px; --inst-band-k1-height: {}px; --inst-band-final-width: {}px; --inst-band-final-height: {}px;",
            square_size, square_size, final_w, final_h
        )
    };
    view! {
        <div class=class style=group_band_dims_style>
            {move || {
                let first_group_channel = first_group_channel();
                let num_keys_per_group = num_keys_per_group();
                let orientation = orientation();
                (0..(num_keys_per_group as usize))
                    .rev()
                    .map(move |k| {
                        {
                            let samples = Signal::derive(move || {
                                batch_data()
                                    .and_then(|b| {
                                        b.snoops
                                            .iter()
                                            .find(|e| e.group as usize == g && e.key as usize == k)
                                            .map(|e| e.samples.clone())
                                    })
                            });
                            view! {
                                <KeyboardElement
                                    g
                                    k
                                    first_group_channel
                                    orientation
                                    activation_samples=samples
                                />
                            }
                        }
                    })
                    .collect_view()
            }}
        </div>
    }
}

#[component]
fn KeyboardElement(
    g: usize,
    k: usize,
    first_group_channel: common::instrument::GroupChannel,
    orientation: common::orientation::LayoutOrientation,
    #[prop(into)] activation_samples: Signal<Option<Vec<f32>>>,
) -> impl IntoView {
    let ctx = expect_instrument_context();
    let LayoutContextReturn {
        key_radius,
        space,
        num_keys_per_group,
        key_band_breadth,
        ..
    } = expect_layout_contex();

    // NodeRefs for band and key wrapper
    let band_ref = NodeRef::<Div>::new();
    let key_ref = NodeRef::<Div>::new();

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
        ..
    } = use_element_bounding(band_ref);

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
        ..
    } = use_element_bounding(key_ref);

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

    // no per-element animation delays; instrument synced globally

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
        let k1_tx = sun_screen_x - (key_x() as f32 + key_width() as f32 / 2.0);
        let k1_ty = sun_screen_y - (key_y() as f32 + key_height() as f32 / 2.0);

        // Breadth-first move: fix the axis orthogonal to main
        let (breadth_tx, breadth_ty) = match orientation {
            common::orientation::LayoutOrientation::Vertical => (0.0, k1_ty),
            common::orientation::LayoutOrientation::Horizontal => (k1_tx, 0.0),
        };

        // Length (main) axis then unstack to final
        let (length_tx, length_ty) = (0.0, 0.0);

        format!(
            concat!(
                "--inst-key-k1-tx: {}px;",
                " --inst-key-k1-ty: {}px;",
                " --inst-key-k1-scale: {};",
                " --inst-key-k2-scale: 1;",
                " --inst-key-breadth-tx: {}px;",
                " --inst-key-breadth-ty: {}px;",
                " --inst-key-length-tx: {}px;",
                " --inst-key-length-ty: {}px;"
            ),
            k1_tx, k1_ty, scale, breadth_tx, breadth_ty, length_tx, length_ty,
        )
    };

    // Classes with one-shot appear animation
    let key_wrapper_class = move || {
        let base = "relative";
        if key_should_animate() {
            format!("{} {}", instrument_animations::INSTRUMENT_KEY_APPEAR, base)
        } else {
            base.to_string()
        }
    };
    let band_class = move || {
        let base = "absolute rounded-full bg-red dark:bg-black border-(length:--keyboard-band-stroke-width) border-black dark:border-red";
        if band_should_animate() {
            format!("{} {}", instrument_animations::INSTRUMENT_BAND_APPEAR, base)
        } else {
            base.to_string()
        }
    };

    let key_code = (g, k);
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
                class="border-none text-thin md:text-base text-sm"
                size=UiSize::Sm
                round=true
                square=true
                attr:id=format!("key-{g}-{k}")
                attr:style=move || {
                    let br = key_band_breadth();
                    let base_r = key_radius();
                    let inc = activation_samples
                        .get()
                        .map(|s| s.iter().map(|v| v.abs()).sum::<f32>())
                        .unwrap_or(0.0)
                        .min(16.0);
                    let dia = ((base_r + inc) * 2.0).min(br.max(0.0));
                    let pad = ((br - dia) / 2.0).max(0.0);
                    format!("width: {}px; height: {}px; margin: {}px;", dia, dia, pad)
                }
            >
                {format!("{key_code:?}")}
            </Button>
        </div>
    }
}
