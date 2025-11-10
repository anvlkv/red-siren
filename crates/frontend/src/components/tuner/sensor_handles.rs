use common::{
    tuner::{Config, Layout as TunerLayout, UpdateSensorPayload},
    NodeKey,
};
use leptos::callback::Callback;
use leptos::{html, prelude::*};
use leptos_use::core::Position;
use leptos_use::{
    use_draggable_with_options, UseDraggableCallbackArgs, UseDraggableOptions, UseDraggableReturn,
};
use mint::Point2;

use crate::{components::with_tooltip, util::setup_context::is_devtools_enabled};

/// Collection of sensor handles
#[component]
pub fn SensorHandles(
    /// Configuration signal
    #[prop(into)]
    config: Signal<Config>,
    /// Tuner layout signal
    #[prop(into)]
    layout: Signal<TunerLayout>,
    /// Callback for updating sensors
    #[prop(into)]
    on_update: Callback<UpdateSensorPayload>,
) -> impl IntoView {
    view! {
        <div class="absolute inset-0 pointer-events-none">
            <div class="relative w-full h-full pointer-events-auto">
                <For each=move || config().sensor_data key=|d| d.key let(child)>
                    <Sensor config key=child.key layout on_update />
                </For>
            </div>
        </div>
    }
}

/// Individual sensor handle with draggable min/max controls
#[component]
fn Sensor(
    /// The sensor data
    #[prop(into)]
    key: NodeKey,
    /// Configuration signal
    #[prop(into)]
    config: Signal<Config>,
    /// Tuner layout signal
    #[prop(into)]
    layout: Signal<TunerLayout>,
    /// Callback for updating sensor
    #[prop(into)]
    on_update: Callback<UpdateSensorPayload>,
) -> impl IntoView {
    let sensor_radius = Memo::new(move |_| layout().sensor_radius);
    let sensor_data = Memo::new(move |_| {
        config()
            .sensor_data
            .into_iter()
            .find(|s| s.key == key)
            .unwrap_or_default()
    });

    view! {
        <div>
            {move || {
                let r = sensor_radius();
                let orientation = layout().orientation;
                let on_move_min = Callback::new(move |Point2 { x, y }| {
                    let config = config();
                    let layout = layout();
                    let data = sensor_data();
                    let (min_frequency, min_magnitude) = config
                        .space_to_frequency_magnitude(&layout, Point2 { x, y });
                    on_update
                        .run(UpdateSensorPayload {
                            key,
                            min_frequency,
                            min_magnitude,
                            max_frequency: data.max_frequency,
                            max_magnitude: data.max_magnitude,
                        });
                });
                let on_move_max = Callback::new(move |Point2 { x, y }| {
                    let config = config();
                    let layout = layout();
                    let data = sensor_data();
                    let (max_frequency, max_magnitude) = config
                        .space_to_frequency_magnitude(&layout, Point2 { x, y });
                    on_update
                        .run(UpdateSensorPayload {
                            key,
                            max_frequency,
                            max_magnitude,
                            min_frequency: data.min_frequency,
                            min_magnitude: data.min_magnitude,
                        });
                });
                let min_pt = Signal::derive(move || {
                    let config = config();
                    let layout = layout();
                    let data = sensor_data();
                    config
                        .frequency_magnitude_to_space(
                            &layout,
                            data.min_frequency,
                            data.min_magnitude,
                        )
                });
                let max_pt = Signal::derive(move || {
                    let config = config();
                    let layout = layout();
                    let data = sensor_data();
                    config
                        .frequency_magnitude_to_space(
                            &layout,
                            data.max_frequency,
                            data.max_magnitude,
                        )
                });

                view! {
                    <Arm
                        key
                        r
                        orientation
                        direction=SensorArmDirection::Min
                        other=max_pt
                        initial_pos=min_pt
                        on_move=on_move_min
                    />
                    <Arm
                        key
                        r
                        orientation
                        direction=SensorArmDirection::Max
                        other=min_pt
                        initial_pos=max_pt
                        on_move=on_move_max
                    />
                }
            }}
            {move || {
                let r = sensor_radius();
                let data = sensor_data();
                let config = config();
                let layout = layout();
                let Point2 { x: min_x, y: min_y } = config
                    .frequency_magnitude_to_space(&layout, data.min_frequency, data.min_magnitude);
                let Point2 { x: max_x, y: max_y } = config
                    .frequency_magnitude_to_space(&layout, data.max_frequency, data.max_magnitude);

                view! { <Connector r max_x max_y min_x min_y /> }
            }}
        </div>
    }
}

#[derive(Debug, Clone, Copy)]
enum SensorArmDirection {
    Min,
    Max,
}

#[component]
/// Directed arm for sensor handle
fn Arm(
    r: f64,
    other: Signal<Point2<f64>>,
    orientation: common::orientation::LayoutOrientation,
    direction: SensorArmDirection,
    key: NodeKey,
    on_move: Callback<Point2<f64>>,
    initial_pos: Signal<Point2<f64>>,
) -> impl IntoView {
    let arm_svg = move || {
        view! {
            <svg
                class="w-full h-full pointer-events-none fill-gray/40 dark:fill-cinnabar/40 stroke-gray dark:stroke-cinnabar stroke-1"
                style="overflow: visible;"
            >
                <path d=format!(
                    "M {m_x} {m_y} A {a_rx} {a_ry} 0 0 {a_sweep_flag} {a_x} {a_y} L {l_x} {l_y} Z",
                    m_x = match orientation {
                        common::orientation::LayoutOrientation::Vertical => 0.0,
                        common::orientation::LayoutOrientation::Horizontal => r,
                    },
                    m_y = match orientation {
                        common::orientation::LayoutOrientation::Vertical => r,
                        common::orientation::LayoutOrientation::Horizontal => 0.0,
                    },
                    a_rx = r,
                    a_ry = r,
                    a_sweep_flag = match (direction, orientation) {
                        (
                            SensorArmDirection::Min,
                            common::orientation::LayoutOrientation::Horizontal,
                        )
                        | (
                            SensorArmDirection::Max,
                            common::orientation::LayoutOrientation::Vertical,
                        ) => 1,
                        _ => 0,
                    },
                    a_x = match orientation {
                        common::orientation::LayoutOrientation::Vertical => r * 2.0,
                        common::orientation::LayoutOrientation::Horizontal => r,
                    },
                    a_y = match orientation {
                        common::orientation::LayoutOrientation::Vertical => r,
                        common::orientation::LayoutOrientation::Horizontal => r * 2.0,
                    },
                    l_x = r,
                    l_y = r,
                ) />
            </svg>
        }
    };

    let handle_ref = NodeRef::<html::Div>::new();
    let with_devtools = is_devtools_enabled();

    let UseDraggableReturn { style, .. } = use_draggable_with_options(
        handle_ref,
        UseDraggableOptions::default()
            .initial_value({
                let pos = initial_pos.get_untracked();
                Position { x: pos.x, y: pos.y }
            })
            .on_move(move |UseDraggableCallbackArgs { position, event }| {
                let allow = match direction {
                    SensorArmDirection::Min => {
                        let other_pos = other();
                        match orientation {
                            common::orientation::LayoutOrientation::Horizontal => {
                                position.x < other_pos.x
                            }
                            common::orientation::LayoutOrientation::Vertical => {
                                position.y < other_pos.y
                            }
                        }
                    }
                    SensorArmDirection::Max => {
                        let other_pos = other();
                        match orientation {
                            common::orientation::LayoutOrientation::Horizontal => {
                                position.x > other_pos.x
                            }
                            common::orientation::LayoutOrientation::Vertical => {
                                position.y > other_pos.y
                            }
                        }
                    }
                };

                if allow {
                    on_move.run(Point2 {
                        x: position.x,
                        y: position.y,
                    });
                } else {
                    event.prevent_default();
                }
            }),
    );

    if with_devtools.get_untracked() {
        with_tooltip(
            handle_ref,
            Signal::derive(move || format!("{key:?} - {direction:?}")),
            Signal::derive(move || None),
        );
    }

    view! {
        <div
            class="relative cursor-move mix-blend-plus-darker dark:mix-blend-plus-lighter"
            node_ref=handle_ref
            style=style
        >
            {arm_svg}
        </div>
    }
}

#[component]
/// Connector line between sensor arms
fn Connector(r: f64, max_x: f64, max_y: f64, min_x: f64, min_y: f64) -> impl IntoView {
    let dx = max_x - min_x;
    let dy = max_y - min_y;
    let len = (dx * dx + dy * dy).sqrt();

    let x1 = min_x + if len > 0.0 { r * dx / len } else { 0.0 };
    let y1 = min_y + if len > 0.0 { r * dy / len } else { 0.0 };
    let x2 = max_x - if len > 0.0 { r * dx / len } else { 0.0 };
    let y2 = max_y - if len > 0.0 { r * dy / len } else { 0.0 };

    view! {
        <svg
            class="absolute inset-0 pointer-events-none mix-blend-plus-darker dark:mix-blend-plus-lighter"
            style="position: absolute; top: 0; left: 0; width: 100%; height: 100%;"
        >
            <line x1=x1 y1=y1 x2=x2 y2=y2 class="stroke-gray/40 dark:stroke-cinnabar/40 stroke-1" />
        </svg>
    }
}
