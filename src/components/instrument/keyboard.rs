use leptos::prelude::*;

use crate::components::Button;

#[component]
pub fn Keyboard(#[prop(into)] layout: Signal<shared::instrument::Layout>) -> impl IntoView {
    let main_container_axis_style = Signal::derive(move || {
        let layout = layout();
        let mut defs = match layout.orientation {
            shared::orientation::LayoutOrientation::Vertical => format!(
                r#"
                --keyboard-rows: repeat({0}, minmax(0, 1fr));
                --keyboard-cols: repeat({1}, minmax(0, 1fr));
                --keyboard-row-gap: {2}px;
                --keyboard-col-gap: {3}px;

                "#,
                layout.num_groups.get(),
                1,
                0,
                layout.groups_gap,
            ),
            shared::orientation::LayoutOrientation::Horizontal => format!(
                r#"
                --keyboard-rows: repeat({0}, minmax(0, 1fr));
                --keyboard-cols: repeat({1}, minmax(0, 1fr));
                --keyboard-row-gap: {2}px;
                --keyboard-col-gap: {3}px;
                "#,
                1,
                layout.num_groups.get(),
                layout.groups_gap,
                0,
            ),
        };

        defs.push_str(&format!("--keyboard-keys-gap: {}px", layout.key_bands_gap));

        defs.push_str(&format!(
            r#"
            --keyboard-band-basis: {}px;
            --keyboard-band-length: {}px;
            "#,
            layout.key_band_breadth, layout.key_band_length
        ));

        defs
    });

    view! {
        <div>
            <div
                style=main_container_axis_style
                class="grid items-center justify-center grid-rows-(--keyboard-rows) grid-cols-(--keyboard-cols) gap-x-(--keyboard-row-gap) gap-y-(--keyboard-col-gap)"
            >
                {move || {
                    let layout = layout();
                    (0..layout.num_groups.get() as usize)
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
        "flex {} gap-(--keyboard-keys-gap)",
        match layout.orientation {
            shared::orientation::LayoutOrientation::Vertical => "flex-col w-full",
            shared::orientation::LayoutOrientation::Horizontal => "flex-row h-full",
        }
    );

    view! {
        <div class=class>
            {move || {
                (0..(layout.num_keys_per_group.get() as usize))
                    .map(move |k| {
                        let key_code = (g, k);

                        view! {
                            <div class="relative basis-(--keyboard-band-basis)">
                                <div
                                    class="absollute w-(--keyboard-band-breadth)"
                                    role="presentation"
                                ></div>
                                <Button class="absollute" square=true round=true>
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
