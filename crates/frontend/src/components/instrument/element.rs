use leptos::{html::Div, prelude::*};
use leptos_use::use_element_bounding;

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
                "--inst-key-k1-tx: {k1_tx}px;",
                " --inst-key-k1-ty: {k1_ty}px;",
                " --inst-key-k1-scale: {scale};",
                " --inst-key-k2-scale: 1;",
                " --inst-key-breadth-tx: {breadth_tx}px;",
                " --inst-key-breadth-ty: {breadth_ty}px;",
                " --inst-key-length-tx: {length_tx}px;",
                " --inst-key-length-ty: {length_ty}px;"
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
        let base = "absolute rounded-full bg-red dark:bg-black border-(length:--keyboard-band-stroke-width) border-black dark:border-red";
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
                    let inc = activation_samples
                        .get()
                        .map(|s| s.iter().map(|v| v.abs()).sum::<f32>())
                        .unwrap_or(0.0)
                        .min(16.0);
                    format!("scale({s}, {s})", s = 0.75 + inc / 64.0)
                }
            >
                {format!("{key_code:?}")}
            </Button>
        </div>
    }
}
