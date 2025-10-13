//! Draggable sensor handles for tuner configuration

use common::orientation::LayoutOrientation;
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
                        // Get current sensor data from config
                        let current_sensor = cfg.sensor_data.get(index).copied().unwrap_or(sensor);

                        // Position sensors based on their frequency and magnitude ranges
                        let line = lay.line_position;
                        let space = lay.space;

                        // Map frequencies to positions along baseline
                        let min_freq = 20.0_f32;
                        let max_freq = 20000.0_f32;
                        let log_min = min_freq.ln();
                        let log_max = max_freq.ln();

                        // Calculate position based on frequency (along baseline)
                        let min_freq_pos = ((current_sensor.min_frequency.ln() - log_min)
                            / (log_max - log_min))
                            .clamp(0.0, 1.0);
                        let max_freq_pos = ((current_sensor.max_frequency.ln() - log_min)
                            / (log_max - log_min))
                            .clamp(0.0, 1.0);

                        // Calculate position based on magnitude (perpendicular to baseline)
                        // Magnitude range is 0-1, map to distance from baseline
                        let max_distance = match lay.orientation {
                            LayoutOrientation::Horizontal => space.y * 0.4, // Use 40% of vertical space
                            LayoutOrientation::Vertical => space.x * 0.4, // Use 40% of horizontal space
                        };

                        let min_mag_distance = current_sensor.min_magnitude * max_distance;
                        let max_mag_distance = current_sensor.max_magnitude * max_distance;

                        // Scale to actual positions
                        let line_length = match lay.orientation {
                            LayoutOrientation::Horizontal => line.1.x - line.0.x,
                            LayoutOrientation::Vertical => line.1.y - line.0.y,
                        };

                        let min_offset = min_freq_pos * line_length;
                        let max_offset = max_freq_pos * line_length;

                        match lay.orientation {
                            LayoutOrientation::Horizontal => (
                                Point2 {
                                    x: line.0.x + min_offset,
                                    y: line.0.y - min_mag_distance, // Above baseline
                                },
                                Point2 {
                                    x: line.0.x + max_offset,
                                    y: line.0.y - max_mag_distance, // Above baseline
                                },
                            ),
                            LayoutOrientation::Vertical => (
                                Point2 {
                                    x: line.0.x + min_mag_distance, // Right of baseline
                                    y: line.0.y + min_offset,
                                },
                                Point2 {
                                    x: line.0.x + max_mag_distance, // Right of baseline
                                    y: line.0.y + max_offset,
                                },
                            ),
                        }
                    })
                })
            })
            .unwrap_or((Point2 { x: 0.0, y: 0.0 }, Point2 { x: 100.0, y: 0.0 }))
    });

    // Handle classes - no border/background, just for interaction
    let handle_class = "relative cursor-move";

    // Create reactive positions that update when config/layout changes
    let min_position = Signal::derive(move || {
        config.with(|c| {
            c.as_ref()
                .and_then(|cfg| {
                    layout.with(|l| {
                        l.as_ref().and_then(|lay| {
                            cfg.sensor_data.get(index).map(|current_sensor| {
                                // Calculate position based on current frequency and magnitude
                                let line = lay.line_position;
                                let space = lay.space;
                                let min_freq = 20.0_f32;
                                let max_freq = 20000.0_f32;
                                let log_min = min_freq.ln();
                                let log_max = max_freq.ln();

                                let freq_pos = ((current_sensor.min_frequency.ln() - log_min)
                                    / (log_max - log_min))
                                    .clamp(0.0, 1.0);

                                let line_length = match lay.orientation {
                                    LayoutOrientation::Horizontal => line.1.x - line.0.x,
                                    LayoutOrientation::Vertical => line.1.y - line.0.y,
                                };

                                let offset = freq_pos * line_length;

                                // Calculate magnitude distance from baseline
                                let max_distance = match lay.orientation {
                                    LayoutOrientation::Horizontal => space.y * 0.4,
                                    LayoutOrientation::Vertical => space.x * 0.4,
                                };
                                let mag_distance = current_sensor.min_magnitude * max_distance;

                                match lay.orientation {
                                    LayoutOrientation::Horizontal => Position {
                                        x: (line.0.x + offset) as f64,
                                        y: (line.0.y - mag_distance) as f64,
                                    },
                                    LayoutOrientation::Vertical => Position {
                                        x: (line.0.x + mag_distance) as f64,
                                        y: (line.0.y + offset) as f64,
                                    },
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
                                // Calculate position based on current frequency and magnitude
                                let line = lay.line_position;
                                let space = lay.space;
                                let min_freq = 20.0_f32;
                                let max_freq = 20000.0_f32;
                                let log_min = min_freq.ln();
                                let log_max = max_freq.ln();

                                let freq_pos = ((current_sensor.max_frequency.ln() - log_min)
                                    / (log_max - log_min))
                                    .clamp(0.0, 1.0);

                                let line_length = match lay.orientation {
                                    LayoutOrientation::Horizontal => line.1.x - line.0.x,
                                    LayoutOrientation::Vertical => line.1.y - line.0.y,
                                };

                                let offset = freq_pos * line_length;

                                // Calculate magnitude distance from baseline
                                let max_distance = match lay.orientation {
                                    LayoutOrientation::Horizontal => space.y * 0.4,
                                    LayoutOrientation::Vertical => space.x * 0.4,
                                };
                                let mag_distance = current_sensor.max_magnitude * max_distance;

                                match lay.orientation {
                                    LayoutOrientation::Horizontal => Position {
                                        x: (line.0.x + offset) as f64,
                                        y: (line.0.y - mag_distance) as f64,
                                    },
                                    LayoutOrientation::Vertical => Position {
                                        x: (line.0.x + mag_distance) as f64,
                                        y: (line.0.y + offset) as f64,
                                    },
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
        let on_select = on_select.clone();
        move |_| {
            on_select.run(Some(index));
        }
    };

    // Min handle draggable
    let UseDraggableReturn {
        x: min_x,
        y: min_y,
        position: min_pos_signal,
        set_position: set_min_position,
        is_dragging: min_dragging,
        style: min_style,
        ..
    } = use_draggable_with_options(
        min_handle_ref,
        UseDraggableOptions::default()
            .initial_value(min_position.get_untracked())
            .on_start({
                let on_select = on_select.clone();
                move |_| {
                    // Select this sensor when starting to drag
                    on_select.run(Some(index));
                    true
                }
            })
            .on_end({
                let on_update = on_update.clone();
                move |UseDraggableCallbackArgs { position, .. }| {
                    // Map position to frequency and magnitude
                    config.with(|c| {
                        layout.with(|l| {
                            if let (Some(cfg), Some(lay)) = (c.as_ref(), l.as_ref()) {
                                let line = lay.line_position;
                                let space = lay.space;

                                // Calculate frequency from position along baseline
                                let line_position = match lay.orientation {
                                    LayoutOrientation::Horizontal => {
                                        ((position.x as f32) - line.0.x) / (line.1.x - line.0.x)
                                    }
                                    LayoutOrientation::Vertical => {
                                        ((position.y as f32) - line.0.y) / (line.1.y - line.0.y)
                                    }
                                }
                                .clamp(0.0, 1.0);

                                // Map to frequency (logarithmic scale)
                                let min_freq = 20.0_f32;
                                let max_freq = 20000.0_f32;
                                let log_min = min_freq.ln();
                                let log_max = max_freq.ln();
                                let log_freq = log_min + (log_max - log_min) * line_position;
                                let freq = log_freq.exp();

                                // Calculate magnitude from distance to baseline
                                let max_distance = match lay.orientation {
                                    LayoutOrientation::Horizontal => space.y * 0.4,
                                    LayoutOrientation::Vertical => space.x * 0.4,
                                };

                                let distance_from_baseline = match lay.orientation {
                                    LayoutOrientation::Horizontal => {
                                        (line.0.y - (position.y as f32)).abs()
                                    }
                                    LayoutOrientation::Vertical => {
                                        ((position.x as f32) - line.0.x).abs()
                                    }
                                };

                                let magnitude =
                                    (distance_from_baseline / max_distance).clamp(0.0, 1.0);

                                // Get current sensor data from config
                                let current_sensor =
                                    cfg.sensor_data.get(index).copied().unwrap_or(sensor);

                                on_update.run(UpdateSensorPayload {
                                    index,
                                    min_frequency: freq,
                                    min_magnitude: magnitude,
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
        position: max_pos_signal,
        set_position: set_max_position,
        is_dragging: max_dragging,
        style: max_style,
        ..
    } = use_draggable_with_options(
        max_handle_ref,
        UseDraggableOptions::default()
            .initial_value(max_position.get_untracked())
            .on_start({
                let on_select = on_select.clone();
                move |_| {
                    // Select this sensor when starting to drag
                    on_select.run(Some(index));
                    true
                }
            })
            .on_end({
                let on_update = on_update.clone();
                move |UseDraggableCallbackArgs { position, .. }| {
                    // Map position to frequency and magnitude
                    config.with(|c| {
                        layout.with(|l| {
                            if let (Some(cfg), Some(lay)) = (c.as_ref(), l.as_ref()) {
                                let line = lay.line_position;
                                let space = lay.space;

                                // Calculate frequency from position along baseline
                                let line_position = match lay.orientation {
                                    LayoutOrientation::Horizontal => {
                                        ((position.x as f32) - line.0.x) / (line.1.x - line.0.x)
                                    }
                                    LayoutOrientation::Vertical => {
                                        ((position.y as f32) - line.0.y) / (line.1.y - line.0.y)
                                    }
                                }
                                .clamp(0.0, 1.0);

                                // Map to frequency (logarithmic scale)
                                let min_freq = 20.0_f32;
                                let max_freq = 20000.0_f32;
                                let log_min = min_freq.ln();
                                let log_max = max_freq.ln();
                                let log_freq = log_min + (log_max - log_min) * line_position;
                                let freq = log_freq.exp();

                                // Calculate magnitude from distance to baseline
                                let max_distance = match lay.orientation {
                                    LayoutOrientation::Horizontal => space.y * 0.4,
                                    LayoutOrientation::Vertical => space.x * 0.4,
                                };

                                let distance_from_baseline = match lay.orientation {
                                    LayoutOrientation::Horizontal => {
                                        (line.0.y - (position.y as f32)).abs()
                                    }
                                    LayoutOrientation::Vertical => {
                                        ((position.x as f32) - line.0.x).abs()
                                    }
                                };

                                let magnitude =
                                    (distance_from_baseline / max_distance).clamp(0.0, 1.0);

                                // Get current sensor data from config
                                let current_sensor =
                                    cfg.sensor_data.get(index).copied().unwrap_or(sensor);

                                on_update.run(UpdateSensorPayload {
                                    index,
                                    min_frequency: current_sensor.min_frequency,
                                    min_magnitude: current_sensor.min_magnitude,
                                    max_frequency: freq,
                                    max_magnitude: magnitude,
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
                on:click=handle_click.clone()
                style=move || {
                    let size = sensor_radius.get() * 2.0;
                    format!(
                        "position: absolute; {}; width: {}px; height: {}px; transform: translate(-{}px, -{}px);",
                        min_style.get(),
                        size,
                        size,
                        sensor_radius.get(),
                        sensor_radius.get(),
                    )
                }
            >
                <svg class="w-full h-full pointer-events-none">
                    // Semi-circle facing left
                    <path
                        d=move || {
                            let r = sensor_radius.get();
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
                        class="fill-gray/40 dark:fill-cinnabar/40 stroke-gray dark:stroke-cinnabar stroke-1"
                    />
                    // Outward line pointing left
                    <line
                        x1=move || sensor_radius.get()
                        y1=move || sensor_radius.get()
                        x2="0"
                        y2=move || sensor_radius.get()
                        class="stroke-gray dark:stroke-cinnabar stroke-2"
                    />
                </svg>
            </div>

            // Max handle (semi-circle with outward line)
            <div
                node_ref=max_handle_ref
                class=move || format!("{} {}", handle_class, opacity_class())
                on:click=handle_click.clone()
                style=move || {
                    let size = sensor_radius.get() * 2.0;
                    format!(
                        "position: absolute; {}; width: {}px; height: {}px; transform: translate(-{}px, -{}px);",
                        max_style.get(),
                        size,
                        size,
                        sensor_radius.get(),
                        sensor_radius.get(),
                    )
                }
            >
                <svg class="w-full h-full pointer-events-none">
                    // Semi-circle facing right
                    <path
                        d=move || {
                            let r = sensor_radius.get();
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
                        class="fill-gray/40 dark:fill-cinnabar/40 stroke-gray dark:stroke-cinnabar stroke-1"
                    />
                    // Outward line pointing right
                    <line
                        x1=move || sensor_radius.get()
                        y1=move || sensor_radius.get()
                        x2=move || sensor_radius.get() * 2.0
                        y2=move || sensor_radius.get()
                        class="stroke-gray dark:stroke-cinnabar stroke-2"
                    />
                </svg>
            </div>

            // Connecting line between handles (visual only, not draggable)
            <svg
                class="absolute inset-0 pointer-events-none"
                style="position: absolute; top: 0; left: 0; width: 100%; height: 100%;"
            >
                <line
                    x1=move || min_x.get()
                    y1=move || min_y.get()
                    x2=move || max_x.get()
                    y2=move || max_y.get()
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
                                    on_update=on_update.clone()
                                    is_active=is_active
                                    on_select=on_select.clone()
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
