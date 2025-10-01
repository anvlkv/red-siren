use leptos::prelude::*;

use crate::components::{Button, UiSize};

const BAND_STROKE_WIDTH: f32 = 4.0;

#[component]
pub fn Keyboard(#[prop(into)] layout: Signal<shared::instrument::Layout>) -> impl IntoView {
    let main_container_axis_style = Signal::derive(move || {
        let layout = layout();

        // Map the orientation-dependent safe area format into CSS box-model (top,right,bottom,left)
        let (safe_top, safe_right, safe_bottom, safe_left) = match layout.orientation {
            // Vertical: indices are [top, left, bottom, right]
            shared::orientation::LayoutOrientation::Vertical => {
                let sa = layout.safe_area_padding;
                (sa[0], sa[3], sa[2], sa[1])
            }
            // Horizontal: indices are [left, top, right, bottom]
            shared::orientation::LayoutOrientation::Horizontal => {
                let sa = layout.safe_area_padding;
                (sa[1], sa[2], sa[3], sa[0])
            }
        };

        let safe_length = layout
            .orientation
            .safe_length(layout.space, layout.safe_area_padding);

        let pad_main = {
            let groups = layout.num_groups.get() as f32;
            let required_length =
                groups * layout.key_band_length + ((groups - 1.0) * layout.groups_gap);

            ((safe_length - required_length) / 2.0).max(0.0)
        };

        let mut defs = match layout.orientation {
            shared::orientation::LayoutOrientation::Vertical => format!(
                r#"
                --keyboard-rows: repeat({0}, minmax(0, 1fr));
                --keyboard-cols: repeat({1}, minmax(0, 1fr));
                --keyboard-row-gap: {2}px;
                --keyboard-col-gap: {3}px;
                --keyboard-pad-x: {4}px;
                --keyboard-pad-y: {5}px;
                "#,
                layout.num_groups.get(),
                1,
                layout.groups_gap,
                0,
                0,
                pad_main
            ),
            shared::orientation::LayoutOrientation::Horizontal => format!(
                r#"
                --keyboard-rows: repeat({0}, minmax(0, 1fr));
                --keyboard-cols: repeat({1}, minmax(0, 1fr));
                --keyboard-row-gap: {2}px;
                --keyboard-col-gap: {3}px;
                --keyboard-pad-x: {4}px;
                --keyboard-pad-y: {5}px;
                "#,
                1,
                layout.num_groups.get(),
                0,
                layout.groups_gap,
                pad_main,
                0
            ),
        };

        defs.push_str(&format!("--keyboard-keys-gap: {}px;", layout.key_bands_gap));

        // Compute padding so the key is exactly centered inside the band.
        let inner_band = layout.key_band_breadth;
        let desired_diameter = layout.key_radius * 2.0;

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
            breadth = layout.key_band_breadth,
            length = layout.key_band_length,
            diameter = { desired_diameter.min(inner_band.max(0.0)) },
            key_pad = {
                // Clamp diameter to the available inner breadth (never negative)
                let dia = desired_diameter.min(inner_band.max(0.0));
                // Evenly distribute the remaining space on both sides to keep the key perfectly centered
                ((inner_band - dia) / 2.0).max(0.0)
            },
            space_w = layout.space.x,
            space_h = layout.space.y,
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
                    let layout = layout();
                    (0..layout.num_groups.get() as usize)
                        .rev()
                        .map(|g| {
                            view! { <Group g layout /> }
                        })
                        .collect_view()
                }}
            </div>
        </div>
    }
}

#[component]
fn Group(layout: shared::instrument::Layout, g: usize) -> impl IntoView {
    let class = format!(
        "flex {} gap-(--keyboard-keys-gap) justify-center items-center",
        match layout.orientation {
            shared::orientation::LayoutOrientation::Vertical => "flex-col w-full",
            shared::orientation::LayoutOrientation::Horizontal => "flex-row h-full",
        }
    );

    view! {
        <div class=class>
            {move || {
                (0..(layout.num_keys_per_group.get() as usize))
                    .rev()
                    .map(move |k| {
                        let key_code = (g, k);
                        let channel_alignment = match layout
                            .first_group_channel
                            .nth_channel_from_first(g)
                        {
                            shared::instrument::GroupChanel::Left => {
                                r#"
                                top: 0;
                                left: 0;
                                "#
                            }
                            shared::instrument::GroupChanel::Right => {
                                r#"
                                bottom: 0;
                                right: 0;
                                "#
                            }
                        };

                        view! {
                            <div class="relative">
                                <div
                                    class="absolute rounded-full bg-red dark:bg-black border-(length:--keyboard-band-stroke-width) border-black dark:border-red"
                                    style=match layout.orientation {
                                        shared::orientation::LayoutOrientation::Vertical => {
                                            format!(
                                                r#"
                                            width: var(--keyboard-band-length);
                                            height: var(--keyboard-band-breadth);

                                            {channel_alignment}
                                            "#,
                                            )
                                        }
                                        shared::orientation::LayoutOrientation::Horizontal => {
                                            format!(
                                                r#"
                                            width: var(--keyboard-band-breadth);
                                            height: var(--keyboard-band-length);

                                            {channel_alignment}
                                            "#,
                                            )
                                        }
                                    }
                                    role="presentation"
                                ></div>
                                <Button
                                    class="border-none text-thin text-base"
                                    size=UiSize::Sm
                                    round=true
                                    square=true
                                    attr:style=r#"
                                    width: var(--keyboard-key-diameter);
                                    height: var(--keyboard-key-diameter);
                                    margin: var(--keyboard-key-padding);
                                    "#
                                >
                                    {format!("{key_code:?}")}
                                </Button>
                            </div>
                        }
                    })
                    .collect_view()
            }}
        </div>
    }
}
