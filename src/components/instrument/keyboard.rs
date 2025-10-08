use common::safe_area::SafeArea;
use leptos::prelude::*;

use crate::{
    components::{Button, UiSize},
    util::layout_context::{expect_layout_contex, LayoutContextReturn},
};

use super::{expect_instrument_context, instrument_animations};
use leptos::html::Div;
use leptos_use::use_element_bounding;

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
                --keyboard-rows: repeat({0}, minmax(0, 1fr));
                --keyboard-cols: repeat({1}, minmax(0, 1fr));
                --keyboard-row-gap: {2}px;
                --keyboard-col-gap: {3}px;
                --keyboard-pad-x: {4}px;
                --keyboard-pad-y: {5}px;
                "#,
                num_groups, 1, groups_gap, 0, 0, pad_main
            ),
            common::orientation::LayoutOrientation::Horizontal => format!(
                r#"
                --keyboard-rows: repeat({0}, minmax(0, 1fr));
                --keyboard-cols: repeat({1}, minmax(0, 1fr));
                --keyboard-row-gap: {2}px;
                --keyboard-col-gap: {3}px;
                --keyboard-pad-x: {4}px;
                --keyboard-pad-y: {5}px;
                "#,
                1, num_groups, 0, groups_gap, pad_main, 0
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
            <div class="grid items-center justify-center grid-rows-(--keyboard-rows) grid-cols-(--keyboard-cols) gap-y-(--keyboard-row-gap) gap-x-(--keyboard-col-gap) p-x-(length:--keyboard-pad-x) p-y-(length:--keyboard-pad-y) w-full h-full">
                {move || {
                    let num_groups = num_groups();
                    (0..num_groups as usize)
                        .rev()
                        .map(|g| {
                            view! { <Group g /> }
                        })
                        .collect_view()
                }}
            </div>
        </div>
    }
}

#[component]
fn Group(g: usize) -> impl IntoView {
    let LayoutContextReturn {
        orientation,
        num_keys_per_group,
        first_group_channel,
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

    view! {
        <div class=class>
            {move || {
                let first_group_channel = first_group_channel();
                let num_keys_per_group = num_keys_per_group();
                let orientation = orientation();
                (0..(num_keys_per_group as usize))
                    .rev()
                    .map(move |k| {
                        {
                            view! { <KeyboardElement g k first_group_channel orientation /> }
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
    first_group_channel: common::instrument::GroupChanel,
    orientation: common::orientation::LayoutOrientation,
) -> impl IntoView {
    let ctx = expect_instrument_context();
    let LayoutContextReturn {
        key_radius,
        key_band_length,
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

    // Stage variables: collapse under sun (k1), glare (k15), then breadth-first and length unstack
    let key_stage1_vars = move || {
        let sun_r = crate::components::intro::consts::INTRO_SUN_RADIUS;
        let scale = sun_r / key_radius();

        // Deterministic glare factor per (g,k) to avoid RNG
        let seed = ((g as u32).wrapping_mul(1315423911)) ^ ((k as u32).wrapping_mul(2654435761));
        let glare_step = (seed % 7) as f32; // 0..6
        let glare = 1.06 + 0.01 * glare_step; // 1.06..1.12

        let k1_tx = -key_x();
        let k1_ty = -key_y();

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
                " --inst-key-k15-scale: {};",
                " --inst-key-k2-scale: 1;",
                " --inst-key-breadth-tx: {}px;",
                " --inst-key-breadth-ty: {}px;",
                " --inst-key-length-tx: {}px;",
                " --inst-key-length-ty: {}px;"
            ),
            k1_tx, k1_ty, scale, glare, breadth_tx, breadth_ty, length_tx, length_ty,
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
        common::instrument::GroupChanel::Left => {
            r#"
            top: 0;
            left: 0;
            "#
        }
        common::instrument::GroupChanel::Right => {
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
    // Make band base square at start by non-uniform scale; collapse under sun and provide glare + staged axis moves
    let band_scale_vars = move || {
        let ratio = (key_band_breadth() / key_band_length()).max(0.0);

        // Deterministic glare per (g,k) — keep circle at Stage 1.5
        let seed = ((g as u32).wrapping_mul(2246822519)) ^ ((k as u32).wrapping_mul(3266489917));
        let glare_step = (seed % 9) as f32; // 0..8
        let glare = 1.06 + 0.01 * glare_step; // 1.06..1.14

        let k1_tx = -band_x();
        let k1_ty = -band_y();

        // Breadth-first move (keep stacked along length)
        let (breadth_tx, breadth_ty) = match orientation {
            common::orientation::LayoutOrientation::Vertical => (0.0, k1_ty),
            common::orientation::LayoutOrientation::Horizontal => (k1_tx, 0.0),
        };

        match orientation {
            common::orientation::LayoutOrientation::Vertical => {
                format!(
                    concat!(
                        "--inst-band-k1-tx: {}px;",
                        " --inst-band-k1-ty: {}px;",
                        " --inst-band-k1-scale-x: {};",
                        " --inst-band-k1-scale-y: 1;",
                        " --inst-band-k15-scale-x: {};",
                        " --inst-band-k15-scale-y: {};",
                        " --inst-band-breadth-tx: {}px;",
                        " --inst-band-breadth-ty: {}px;"
                    ),
                    k1_tx, k1_ty, ratio, glare, glare, breadth_tx, breadth_ty
                )
            }
            common::orientation::LayoutOrientation::Horizontal => {
                format!(
                    concat!(
                        "--inst-band-k1-tx: {}px;",
                        " --inst-band-k1-ty: {}px;",
                        " --inst-band-k1-scale-x: 1;",
                        " --inst-band-k1-scale-y: {};",
                        " --inst-band-k15-scale-x: {};",
                        " --inst-band-k15-scale-y: {};",
                        " --inst-band-breadth-tx: {}px;",
                        " --inst-band-breadth-ty: {}px;"
                    ),
                    k1_tx, k1_ty, ratio, glare, glare, breadth_tx, breadth_ty
                )
            }
        }
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
                class="border-none text-thin text-base"
                size=UiSize::Sm
                round=true
                square=true
                attr:id=format!("key-{g}-{k}")
                attr:style="width: var(--keyboard-key-diameter);\
                height: var(--keyboard-key-diameter);\
                margin: var(--keyboard-key-padding);\
                "
            >
                {format!("{key_code:?}")}
            </Button>
        </div>
    }
}
