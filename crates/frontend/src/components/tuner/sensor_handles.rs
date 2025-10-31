//! Draggable sensor handles for tuner configuration

use common::tuner::{Config, Layout as TunerLayout, SensorData, UpdateSensorPayload};
use leptos::callback::Callback;
use leptos::{html, prelude::*};
use leptos_use::core::Position;
use leptos_use::{
    use_draggable_with_options, UseDraggableCallbackArgs, UseDraggableOptions, UseDraggableReturn,
};
use mint::Point2;

/// Individual sensor handle with draggable min/max controls
#[component]
pub fn SensorHandle(
    /// The sensor data
    #[prop(into)]
    sensor: SensorData,
    /// Index of this sensor
    #[prop(into)]
    index: usize,
    /// Configuration signal
    config: Signal<Option<Config>>,
    /// Tuner layout signal
    layout: Signal<Option<TunerLayout>>,
    /// Callback for updating sensor
    on_update: Callback<UpdateSensorPayload>,
    /// Whether this sensor is active/selected
    is_active: Signal<bool>,
    /// Callback for selecting this sensor
    on_select: Callback<Option<usize>>,
) -> impl IntoView {
    let min_handle_ref = NodeRef::<html::Div>::new();
    let max_handle_ref = NodeRef::<html::Div>::new();

    // Get sensor_radius from layout
    let sensor_radius = Memo::new(move |_| {
        layout
            .with(|l| l.as_ref().map(|lay| lay.sensor_radius))
            .unwrap_or(10.0)
    });

    // Calculate initial positions based on current sensor frequency and magnitude ranges
    let (initial_min_pos, initial_max_pos) = config.with(|c| {
        c.as_ref()
            .and_then(|cfg| {
                layout.with(|l| {
                    l.as_ref().map(|lay| {
                        let current_sensor = cfg.sensor_data.get(index).copied().unwrap_or(sensor);
                        let p_min = cfg.frequency_magnitude_to_space(
                            lay,
                            current_sensor.min_frequency,
                            current_sensor.min_magnitude,
                        );
                        let p_max = cfg.frequency_magnitude_to_space(
                            lay,
                            current_sensor.max_frequency,
                            current_sensor.max_magnitude,
                        );
                        (p_min, p_max)
                    })
                })
            })
            .unwrap_or((Point2 { x: 0.0, y: 0.0 }, Point2 { x: 100.0, y: 0.0 }))
    });

    // Handle classes - no border/background, just for interaction
    let handle_class = "relative cursor-move mix-blend-difference dark:mix-blend-exclusion";

    // Create reactive positions that update when config/layout changes
    let min_position = Signal::derive(move || {
        config.with(|c| {
            c.as_ref()
                .and_then(|cfg| {
                    layout.with(|l| {
                        l.as_ref().and_then(|lay| {
                            cfg.sensor_data.get(index).map(|current_sensor| {
                                let p = cfg.frequency_magnitude_to_space(
                                    lay,
                                    current_sensor.min_frequency,
                                    current_sensor.min_magnitude,
                                );
                                Position {
                                    x: p.x as f64,
                                    y: p.y as f64,
                                }
                            })
                        })
                    })
                })
                .unwrap_or(Position {
                    x: initial_min_pos.x as f64,
                    y: initial_min_pos.y as f64,
                })
        })
    });

    let max_position = Signal::derive(move || {
        config.with(|c| {
            c.as_ref()
                .and_then(|cfg| {
                    layout.with(|l| {
                        l.as_ref().and_then(|lay| {
                            cfg.sensor_data.get(index).map(|current_sensor| {
                                let p = cfg.frequency_magnitude_to_space(
                                    lay,
                                    current_sensor.max_frequency,
                                    current_sensor.max_magnitude,
                                );
                                Position {
                                    x: p.x as f64,
                                    y: p.y as f64,
                                }
                            })
                        })
                    })
                })
                .unwrap_or(Position {
                    x: initial_max_pos.x as f64,
                    y: initial_max_pos.y as f64,
                })
        })
    });

    // Click handler for selecting sensor
    let handle_click = {
        move |_| {
            on_select.run(Some(index));
        }
    };

    // Min handle draggable
    let UseDraggableReturn {
        x: min_x,
        y: min_y,
        position: _,
        set_position: set_min_position,
        is_dragging: min_dragging,
        style: min_style,
        ..
    } = use_draggable_with_options(
        min_handle_ref,
        UseDraggableOptions::default()
            .initial_value(min_position.get_untracked())
            .on_start({
                move |_| {
                    // Select this sensor when starting to drag
                    on_select.run(Some(index));
                    true
                }
            })
            .on_end({
                move |UseDraggableCallbackArgs { position, .. }| {
                    // Map position to frequency and magnitude using config
                    config.with(|c| {
                        layout.with(|l| {
                            if let (Some(cfg), Some(lay)) = (c.as_ref(), l.as_ref()) {
                                let (freq, magnitude) = cfg.space_to_frequency_magnitude(
                                    lay,
                                    mint::Point2 {
                                        x: position.x as f32,
                                        y: position.y as f32,
                                    },
                                );
                                let current_sensor =
                                    cfg.sensor_data.get(index).copied().unwrap_or(sensor);

                                // Constrain min values to be less than max values
                                let constrained_min_freq =
                                    freq.min(current_sensor.max_frequency - 1.0);
                                let constrained_min_mag =
                                    magnitude.min(current_sensor.max_magnitude - 1.0);

                                on_update.run(UpdateSensorPayload {
                                    key: current_sensor.key,
                                    min_frequency: constrained_min_freq,
                                    min_magnitude: constrained_min_mag,
                                    max_frequency: current_sensor.max_frequency,
                                    max_magnitude: current_sensor.max_magnitude,
                                });
                            }
                        });
                    });
                }
            }),
    );

    // Sync handle position from config while not dragging
    Effect::new(move |_| {
        let target = min_position.get();
        if !min_dragging.get() {
            set_min_position.set(target);
        }
    });

    // Don't update during dragging - only on mount

    // Max handle draggable
    let UseDraggableReturn {
        x: max_x,
        y: max_y,
        position: _,
        set_position: set_max_position,
        is_dragging: max_dragging,
        style: max_style,
        ..
    } = use_draggable_with_options(
        max_handle_ref,
        UseDraggableOptions::default()
            .initial_value(max_position.get_untracked())
            .on_start({
                move |_| {
                    // Select this sensor when starting to drag
                    on_select.run(Some(index));
                    true
                }
            })
            .on_end({
                move |UseDraggableCallbackArgs { position, .. }| {
                    // Map position to frequency and magnitude using config
                    config.with(|c| {
                        layout.with(|l| {
                            if let (Some(cfg), Some(lay)) = (c.as_ref(), l.as_ref()) {
                                let (freq, magnitude) = cfg.space_to_frequency_magnitude(
                                    lay,
                                    mint::Point2 {
                                        x: position.x as f32,
                                        y: position.y as f32,
                                    },
                                );
                                let current_sensor =
                                    cfg.sensor_data.get(index).copied().unwrap_or(sensor);

                                // Constrain max values to be greater than min values
                                let constrained_max_freq =
                                    freq.max(current_sensor.min_frequency + 1.0);
                                let constrained_max_mag =
                                    magnitude.max(current_sensor.min_magnitude + 1.0);

                                on_update.run(UpdateSensorPayload {
                                    key: current_sensor.key,
                                    min_frequency: current_sensor.min_frequency,
                                    min_magnitude: current_sensor.min_magnitude,
                                    max_frequency: constrained_max_freq,
                                    max_magnitude: constrained_max_mag,
                                });
                            }
                        });
                    });
                }
            }),
    );

    // Sync handle position from config while not dragging
    Effect::new(move |_| {
        let target = max_position.get();
        if !max_dragging.get() {
            set_max_position.set(target);
        }
    });

    // Opacity based on active state
    let opacity_class = move || {
        if is_active.get() {
            "opacity-100"
        } else {
            "opacity-60 hover:opacity-80"
        }
    };

    view! {
        <>
            // Min handle (semi-circle with outward line)
            <div
                node_ref=min_handle_ref
                class=move || format!("{} {}", handle_class, opacity_class())
                on:click=handle_click
                style=move || {
                    let stroke_pad = 2.0;
                    let size = sensor_radius.get() * 2.0 + stroke_pad;
                    let offset = sensor_radius.get() + stroke_pad / 2.0;
                    format!(
                        "position: absolute; {}; width: {}px; height: {}px; transform: translate(-{}px, -{}px); overflow: visible;",
                        min_style.get(),
                        size,
                        size,
                        offset,
                        offset,
                    )
                }
            >
                <svg class="w-full h-full pointer-events-none" style="overflow: visible;">
                    // Semi-circle facing left (horizontal) or up (vertical)
                    <path
                        d=move || {
                            let r = sensor_radius.get();
                            layout
                                .with(|l| {
                                    match l.as_ref().map(|lay| lay.orientation) {
                                        Some(common::orientation::LayoutOrientation::Vertical) => {
                                            format!(
                                                "M {} {} A {} {} 0 0 0 {} {} L {} {} Z",
                                                0.0,
                                                r,
                                                r,
                                                r,
                                                r * 2.0,
                                                r,
                                                r,
                                                r,
                                            )
                                        }
                                        _ => {
                                            format!(
                                                "M {} {} A {} {} 0 0 1 {} {} L {} {} Z",
                                                r,
                                                0.0,
                                                r,
                                                r,
                                                r,
                                                r * 2.0,
                                                r,
                                                r,
                                            )
                                        }
                                    }
                                })
                        }
                        class="fill-gray/40 dark:fill-cinnabar/40 stroke-gray dark:stroke-cinnabar stroke-1"
                    />
                </svg>
            </div>

            // Max handle (semi-circle with outward line)
            <div
                node_ref=max_handle_ref
                class=move || format!("{} {}", handle_class, opacity_class())
                on:click=handle_click
                style=move || {
                    let stroke_pad = 2.0;
                    let size = sensor_radius.get() * 2.0 + stroke_pad;
                    let offset = sensor_radius.get() + stroke_pad / 2.0;
                    format!(
                        "position: absolute; {}; width: {}px; height: {}px; transform: translate(-{}px, -{}px); overflow: visible;",
                        max_style.get(),
                        size,
                        size,
                        offset,
                        offset,
                    )
                }
            >
                <svg class="w-full h-full pointer-events-none" style="overflow: visible;">
                    // Semi-circle facing right (horizontal) or down (vertical)
                    <path
                        d=move || {
                            let r = sensor_radius.get();
                            layout
                                .with(|l| {
                                    match l.as_ref().map(|lay| lay.orientation) {
                                        Some(common::orientation::LayoutOrientation::Vertical) => {
                                            format!(
                                                "M {} {} A {} {} 0 0 1 {} {} L {} {} Z",
                                                0.0,
                                                r,
                                                r,
                                                r,
                                                r * 2.0,
                                                r,
                                                r,
                                                r,
                                            )
                                        }
                                        _ => {
                                            format!(
                                                "M {} {} A {} {} 0 0 0 {} {} L {} {} Z",
                                                r,
                                                0.0,
                                                r,
                                                r,
                                                r,
                                                r * 2.0,
                                                r,
                                                r,
                                            )
                                        }
                                    }
                                })
                        }
                        class="fill-gray/40 dark:fill-cinnabar/40 stroke-gray dark:stroke-cinnabar stroke-1"
                    />
                </svg>
            </div>

            // Connecting line between handles (visual only, not draggable)
            <svg
                class="absolute inset-0 pointer-events-none mix-blend-difference dark:mix-blend-exclusion"
                style="position: absolute; top: 0; left: 0; width: 100%; height: 100%;"
            >
                <line
                    x1=move || {
                        let r = (sensor_radius.get() - 1.0) as f64;
                        let dx = max_x.get() - min_x.get();
                        let dy = max_y.get() - min_y.get();
                        let len = (dx * dx + dy * dy).sqrt();
                        min_x.get() + if len > 0.0 { r * dx / len } else { 0.0 }
                    }
                    y1=move || {
                        let r = (sensor_radius.get() - 1.0) as f64;
                        let dx = max_x.get() - min_x.get();
                        let dy = max_y.get() - min_y.get();
                        let len = (dx * dx + dy * dy).sqrt();
                        min_y.get() + if len > 0.0 { r * dy / len } else { 0.0 }
                    }
                    x2=move || {
                        let r = (sensor_radius.get() - 1.0) as f64;
                        let dx = max_x.get() - min_x.get();
                        let dy = max_y.get() - min_y.get();
                        let len = (dx * dx + dy * dy).sqrt();
                        max_x.get() - if len > 0.0 { r * dx / len } else { 0.0 }
                    }
                    y2=move || {
                        let r = (sensor_radius.get() - 1.0) as f64;
                        let dx = max_x.get() - min_x.get();
                        let dy = max_y.get() - min_y.get();
                        let len = (dx * dx + dy * dy).sqrt();
                        max_y.get() - if len > 0.0 { r * dy / len } else { 0.0 }
                    }
                    class="stroke-gray/40 dark:stroke-cinnabar/40 stroke-1"
                />
            </svg>
        </>
    }
}

/// Collection of sensor handles
#[component]
pub fn SensorHandles(
    /// Configuration signal
    config: Signal<Option<Config>>,
    /// Tuner layout signal
    layout: Signal<Option<TunerLayout>>,
    /// Callback for updating sensors
    on_update: Callback<UpdateSensorPayload>,
    /// Currently active sensor index
    active_sensor: Signal<Option<usize>>,
    /// Callback for selecting a sensor
    on_select: Callback<Option<usize>>,
) -> impl IntoView {
    let handles = move || {
        config.with(|c| {
            c.as_ref()
                .map(|cfg| {
                    cfg.sensor_data
                        .iter()
                        .enumerate()
                        .map(|(index, sensor)| {
                            let is_active = Signal::derive(move || {
                                active_sensor.with(|active| active == &Some(index))
                            });

                            view! {
                                <SensorHandle
                                    sensor=*sensor
                                    index=index
                                    config=config
                                    layout=layout
                                    on_update=on_update
                                    is_active=is_active
                                    on_select=on_select
                                />
                            }
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        })
    };

    view! {
        <div class="absolute inset-0 pointer-events-none">
            <div class="relative w-full h-full pointer-events-auto">{handles}</div>
        </div>
    }
}
