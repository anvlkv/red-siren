//! Spectrum visualizer component for displaying FFT analysis results

use leptos::prelude::*;

use common::tuner::{Config, Layout as TunerLayout, SpectrumData};

/// Spectrum visualizer with three opacity layers
#[component]
pub fn SpectrumVisualizer(
    /// Spectrum data signal
    spectrum: Signal<Option<SpectrumData>>,

    /// Tuner layout (space, orientation, baseline, sensors count)
    layout: Signal<Option<TunerLayout>>,
) -> impl IntoView {
    // Get baseline from layout (no fallback)
    let baseline = Memo::new(move |_| layout.with(|l| l.as_ref().map(|lay| lay.line_position)));

    // Generate current spectrum path
    let current_path = Memo::new(move |_| {
        spectrum.with(|data| {
            data.as_ref().and_then(|d| {
                layout.with(|l| {
                    l.as_ref().map(|lay| {
                        generate_spectrum_path(
                            &d.current_magnitudes,
                            &d.frequencies,
                            lay,
                            d.sample_rate,
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
                layout.with(|l| {
                    l.as_ref().map(|lay| {
                        generate_spectrum_path(
                            &d.max_magnitudes,
                            &d.frequencies,
                            lay,
                            d.sample_rate,
                        )
                    })
                })
            })
        })
    });

    let activations = Signal::derive(move || spectrum().map(|data| data.sensor_activations));
    // fft_size not needed for dB scaling

    view! {
        <svg class="absolute inset-0 w-full h-full">
            // Baseline reference line
            <Show when=move || baseline.get().is_some()>
                <line
                    x1=move || baseline.get().unwrap().0.x
                    y1=move || baseline.get().unwrap().0.y
                    x2=move || baseline.get().unwrap().1.x
                    y2=move || baseline.get().unwrap().1.y
                    class="stroke-gray/20 dark:stroke-cinnabar/20 stroke-1"
                />
            </Show>

            // Sensor activation bars (highest opacity - 60%)
            <Show when=move || spectrum().is_some() && layout().is_some()>
                <g class="fill-gray/20 dark:fill-cinnabar/20" style="mix-blend-mode: multiply">
                    <ActivationBars activations=activations layout=layout baseline=baseline />
                </g>
            </Show>

            // Max hold layer (lowest opacity - 10%)
            <Show when=move || max_path.get().is_some()>
                <path
                    d=move || max_path.get().unwrap_or_default()
                    class="fill-gray/30 dark:fill-cinnabar/30 stroke-gray/50 dark:stroke-cinnabar/50 stroke-1"
                    style="mix-blend-mode: multiply"
                />
            </Show>

            // Current spectrum layer (medium opacity - 20%)
            <Show when=move || current_path.get().is_some()>
                <path
                    d=move || current_path.get().unwrap_or_default()
                    class="fill-gray/40 dark:fill-cinnabar/40 stroke-gray/60 dark:stroke-cinnabar/60 stroke-1"
                    style="mix-blend-mode: multiply"
                />
            </Show>
        </svg>
    }
}

/// Generate SVG path for spectrum data using Config mapping
fn generate_spectrum_path(
    magnitudes: &[f32],
    frequencies: &[f32],
    layout: &TunerLayout,
    sample_rate: f32,
) -> String {
    if magnitudes.is_empty() || frequencies.is_empty() {
        return String::new();
    }

    let mut path = String::new();
    let baseline = layout.line_position;

    // Use Config mapping for coordinates
    let cfg = Config {
        sensor_data: vec![],
        fft_size: 0,
        sample_rate,
    };

    // Start from the left edge at baseline level
    path.push_str(&format!("M {} {} ", 0.0, baseline.0.y));

    for (i, &mag) in magnitudes.iter().enumerate() {
        if i < frequencies.len() {
            let p = cfg.frequency_magnitude_to_space(layout, frequencies[i], mag);
            path.push_str(&format!("L {} {} ", p.x, p.y));
        }
    }

    // Close path back to baseline at the right edge
    path.push_str(&format!("L {} {} ", layout.space.x, baseline.0.y));
    path.push_str(&format!("L {} {} ", 0.0, baseline.0.y));

    path.push_str(" Z");

    path
}

/// Generate sensor activation bars
#[component]
fn ActivationBars(
    #[prop(into)] activations: Signal<Option<Vec<f32>>>,
    #[prop(into)] layout: Signal<Option<TunerLayout>>,
    #[prop(into)] baseline: Signal<Option<common::Line>>,
) -> impl IntoView {
    use common::orientation::LayoutOrientation;

    // Calculate bar dimensions based on number of sensors
    let num_sensors = move || layout().as_ref().unwrap().num_sensors.get() as usize;
    let line_length = move || {
        let layout = *layout().as_ref().unwrap();
        let baseline = baseline().unwrap();
        match layout.orientation {
            LayoutOrientation::Horizontal => (baseline.1.x - baseline.0.x).abs(),
            LayoutOrientation::Vertical => (baseline.1.y - baseline.0.y).abs(),
        }
    };
    let bar_width = move || line_length() / (num_sensors() as f32);
    let bar_spacing = move || bar_width() * 0.1; // 10% spacing between bars
                                                 // Calculate scaling factor to fit all activations within 0..1 range
    let scale_factor = move || {
        activations()
            .as_ref()
            .map(|acts| {
                let max_activation = acts.iter().fold(0.0f32, |acc, &x| acc.max(x.abs()));
                if max_activation > 1.0 {
                    1.0 / max_activation
                } else {
                    1.0
                }
            })
            .unwrap_or(1.0)
    };
    let level = move |v: f32| (v * scale_factor()).clamp(0.0, 1.0);

    view! {
        <>
            {move || {
                activations()
                    .as_ref()
                    .unwrap()
                    .iter()
                    .enumerate()
                    .map(|(i, &activation)| {
                        let layout = *layout().as_ref().unwrap();
                        let baseline = baseline().unwrap();
                        let bar_width = bar_width();
                        let bar_spacing = bar_spacing();
                        let (x, y, width, height) = match layout.orientation {
                            LayoutOrientation::Horizontal => {
                                let x = baseline.0.x + (i as f32 * bar_width) + bar_spacing / 2.0;
                                let avail_up = baseline.0.y;
                                let bar_height = level(activation) * avail_up;
                                let y = baseline.0.y - bar_height;
                                let width = bar_width - bar_spacing;
                                (x, y, width, bar_height)
                            }
                            LayoutOrientation::Vertical => {
                                let y = baseline.0.y + (i as f32 * bar_width) + bar_spacing / 2.0;
                                let avail_right = layout.space.x - baseline.0.x;
                                let bar_width_actual = level(activation) * avail_right;
                                let x = baseline.0.x;
                                let height = bar_width - bar_spacing;
                                (x, y, bar_width_actual, height)
                            }
                        };
                        // no sqrtN; activations are already 0..1

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
                    })
                    .collect_view()
            }}
        </>
    }
}
