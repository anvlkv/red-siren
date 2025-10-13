//! Spectrum visualizer component for displaying FFT analysis results

use leptos::prelude::*;

use common::tuner::{Config, SpectrumData};

/// Spectrum visualizer with three opacity layers
#[component]
pub fn SpectrumVisualizer(
    /// Spectrum data signal
    spectrum: Signal<Option<SpectrumData>>,
    /// Tuner configuration
    config: Signal<Option<Config>>,
) -> impl IntoView {
    // Get baseline from config
    let baseline = Memo::new(move |_| {
        config.with(|c| {
            c.as_ref()
                .map(|cfg| cfg.layout.line_position)
                .unwrap_or_else(|| {
                    (
                        mint::Point2 { x: 20.0, y: 120.0 },
                        mint::Point2 { x: 300.0, y: 120.0 },
                    )
                })
        })
    });

    // Generate current spectrum path
    let current_path = Memo::new(move |_| {
        spectrum.with(|data| {
            data.as_ref().and_then(|d| {
                config.with(|c| {
                    c.as_ref().map(|cfg| {
                        generate_spectrum_path(
                            &d.current_magnitudes,
                            &d.frequencies,
                            cfg,
                            baseline.get(),
                        )
                    })
                })
            })
        })
    });

    // Generate max hold path
    let max_path = Memo::new(move |_| {
        spectrum.with(|data| {
            data.as_ref().and_then(|d| {
                config.with(|c| {
                    c.as_ref().map(|cfg| {
                        generate_spectrum_path(
                            &d.max_magnitudes,
                            &d.frequencies,
                            cfg,
                            baseline.get(),
                        )
                    })
                })
            })
        })
    });

    view! {
        <svg class="absolute inset-0 w-full h-full">
            // Baseline reference line
            <line
                x1=move || baseline.get().0.x
                y1=move || baseline.get().0.y
                x2=move || baseline.get().1.x
                y2=move || baseline.get().1.y
                class="stroke-gray/20 dark:stroke-cinnabar/20 stroke-1"
            />

            // Max hold layer (lowest opacity - 10%)
            <Show when=move || max_path.get().is_some()>
                <path
                    d=move || max_path.get().unwrap_or_default()
                    class="fill-gray/10 dark:fill-cinnabar/10 stroke-gray/20 dark:stroke-cinnabar/20 stroke-1"
                />
            </Show>

            // Current spectrum layer (medium opacity - 20%)
            <Show when=move || current_path.get().is_some()>
                <path
                    d=move || current_path.get().unwrap_or_default()
                    class="fill-gray/20 dark:fill-cinnabar/20 stroke-gray/40 dark:stroke-cinnabar/40 stroke-1"
                />
            </Show>

            // Sensor activation bars (highest opacity - 60%)
            <g class="fill-gray/60 dark:fill-cinnabar/60">
                {move || {
                    spectrum
                        .with(|data| {
                            data.as_ref()
                                .and_then(|d| {
                                    config
                                        .with(|c| {
                                            c.as_ref()
                                                .map(|cfg| {

                                                    // generate_activation_bars(&d.sensor_activations, cfg, baseline.get())
                                                    view! {
                                                        <ActivationBars
                                                            activations=d.sensor_activations.clone()
                                                            config=cfg.clone()
                                                            baseline=baseline.get()
                                                        />
                                                    }
                                                })
                                        })
                                })
                        })
                }}
            </g>
        </svg>
    }
}

/// Generate SVG path for spectrum data relative to baseline
fn generate_spectrum_path(
    magnitudes: &[f32],
    frequencies: &[f32],
    config: &Config,
    baseline: common::Line,
) -> String {
    use common::orientation::LayoutOrientation;

    if magnitudes.is_empty() || frequencies.is_empty() {
        return String::new();
    }

    let mut path = String::new();

    // Start from baseline origin
    path.push_str(&format!("M {} {} ", baseline.0.x, baseline.0.y));

    // Calculate path points based on orientation
    match config.layout.orientation {
        LayoutOrientation::Horizontal => {
            // Frequency on X axis, magnitude on Y axis
            let baseline_y = baseline.0.y;
            let line_length = (baseline.1.x - baseline.0.x).abs();

            for (i, &mag) in magnitudes.iter().enumerate() {
                if i < frequencies.len() {
                    // Map frequency to position along baseline
                    let freq_ratio = frequencies[i] / (config.sample_rate / 2.0);
                    let x = baseline.0.x + (freq_ratio * line_length);
                    // Magnitude extends from baseline
                    let y = baseline_y - (mag * config.layout.space.y * 0.5); // Scale to half height

                    if i == 0 {
                        path.push_str(&format!("L {} {} ", x, y));
                    } else {
                        path.push_str(&format!("L {} {} ", x, y));
                    }
                }
            }

            // Close path back to baseline
            path.push_str(&format!("L {} {} ", baseline.1.x, baseline.1.y));
            path.push_str(&format!("L {} {} Z", baseline.0.x, baseline.0.y));
        }
        LayoutOrientation::Vertical => {
            // Frequency on Y axis, magnitude on X axis
            let baseline_x = baseline.0.x;
            let line_length = (baseline.1.y - baseline.0.y).abs();

            for (i, &mag) in magnitudes.iter().enumerate() {
                if i < frequencies.len() {
                    // Map frequency to position along baseline
                    let freq_ratio = frequencies[i] / (config.sample_rate / 2.0);
                    let y = baseline.0.y + (freq_ratio * line_length);
                    // Magnitude extends from baseline
                    let x = baseline_x + (mag * config.layout.space.x * 0.5); // Scale to half width

                    if i == 0 {
                        path.push_str(&format!("L {} {} ", x, y));
                    } else {
                        path.push_str(&format!("L {} {} ", x, y));
                    }
                }
            }

            // Close path back to baseline
            path.push_str(&format!("L {} {} ", baseline.1.x, baseline.1.y));
            path.push_str(&format!("L {} {} Z", baseline.0.x, baseline.0.y));
        }
    }

    path
}

/// Generate sensor activation bars
#[component]
fn ActivationBars(
    #[prop(into)] activations: Vec<f32>,
    #[prop(into)] config: Config,
    #[prop(into)] baseline: common::Line,
) -> impl IntoView {
    use common::orientation::LayoutOrientation;

    let mut bars = Vec::new();

    // Calculate bar dimensions based on number of sensors
    let num_sensors = config.layout.num_sensors.get() as usize;
    if num_sensors == 0 || activations.len() < num_sensors {
        return bars;
    }

    let line_length = match config.layout.orientation {
        LayoutOrientation::Horizontal => (baseline.1.x - baseline.0.x).abs(),
        LayoutOrientation::Vertical => (baseline.1.y - baseline.0.y).abs(),
    };

    let bar_width = line_length / (num_sensors as f32);
    let bar_spacing = bar_width * 0.1; // 10% spacing between bars

    for (i, _) in config.sensor_data.iter().enumerate() {
        if i >= activations.len() {
            break;
        }

        let activation = activations[i];
        if activation <= 0.0 {
            continue; // Skip inactive sensors
        }

        // Calculate bar position and size based on orientation
        let (x, y, width, height) = match config.layout.orientation {
            LayoutOrientation::Horizontal => {
                let x = baseline.0.x + (i as f32 * bar_width) + bar_spacing / 2.0;
                let bar_height = activation * config.layout.space.y * 0.3; // Scale to 30% of space
                let y = baseline.0.y - bar_height;
                let width = bar_width - bar_spacing;
                (x, y, width, bar_height)
            }
            LayoutOrientation::Vertical => {
                let y = baseline.0.y + (i as f32 * bar_width) + bar_spacing / 2.0;
                let bar_width_actual = activation * config.layout.space.x * 0.3; // Scale to 30% of space
                let x = baseline.0.x;
                let height = bar_width - bar_spacing;
                (x, y, bar_width_actual, height)
            }
        };

        // Create rect element for this sensor
        bars.push(
            view! {
                <rect
                    x=x
                    y=y
                    width=width
                    height=height
                    rx="2"
                    ry="2"
                    class="transition-all duration-75"
                />
            }
            .into_view(),
        );
    }

    bars.collect_view()
}
