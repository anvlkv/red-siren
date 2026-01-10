mod context;
mod sensor_handles;
mod spectrum;

use common::tuner::{Config, ReflectTunerConstraints, UpdateRangePayload, UpdateSensorPayload};
use leptos::callback::Callback;
use leptos::prelude::*;
use leptos_use::{use_device_pixel_ratio, use_element_size, UseElementSizeReturn};
use tauri_use::{use_invoke, UseTauriReturn};

pub use context::{expect_tuner_service, provide_tuner_service};
pub use sensor_handles::SensorHandles;
pub use spectrum::SpectrumVisualizer;

use crate::components::{RangeSlider, SliderValue, UiSize};

const OVERLAY_BAR_MIN_PX: f64 = 24.0;
const OVERLAY_BAR_SCALE_RATIO: f64 = 0.08;
const OVERLAY_TRANSITION_MS: u32 = 200;

/// Canonical canvas transform shared by all tuner canvases
#[derive(Clone, PartialEq)]
pub struct CanvasTransform {
    /// CSS px per world unit
    pub world_scale_css: f64,
    /// device px per world unit (uniform, for backward compatibility)
    pub device_scale: f64,
    /// device px per world unit along X (safe-area scaled)
    pub device_scale_x: f64,
    /// device px per world unit along Y (safe-area scaled)
    pub device_scale_y: f64,
    /// device px offset X for left/top margins (margin-aware)
    pub translate_x_margin_dev: f64,
    /// device px offset Y for left/top margins (margin-aware)
    pub translate_y_margin_dev: f64,
    /// CSS px left margin due to min-frequency overlay
    pub margin_left_css: f64,
    /// CSS px right margin due to max-frequency overlay
    pub margin_right_css: f64,
    /// CSS px top margin due to min-frequency overlay (vertical)
    pub margin_top_css: f64,
    /// CSS px bottom margin due to max-frequency overlay (vertical)
    pub margin_bottom_css: f64,
    /// device px width
    pub backing_width_dev: u32,
    /// device px height
    pub backing_height_dev: u32,
    /// DPR
    pub device_pixel_ratio: f64,
}

impl CanvasTransform {
    pub fn css_to_world(&self, css_x: f64, css_y: f64) -> (f64, f64) {
        let dev_x = css_x * self.device_pixel_ratio;
        let dev_y = css_y * self.device_pixel_ratio;
        let world_x = (dev_x - self.translate_x_margin_dev) / self.device_scale_x;
        let world_y = (dev_y - self.translate_y_margin_dev) / self.device_scale_y;
        (world_x, world_y)
    }

    pub fn css_to_device(&self, css_x: f64, css_y: f64) -> (f64, f64) {
        let dev_x = css_x * self.device_pixel_ratio;
        let dev_y = css_y * self.device_pixel_ratio;
        (dev_x, dev_y)
    }
}

/// Main tuner component with spectrum visualization and sensor configuration
#[component]
pub fn Tuner() -> impl IntoView {
    let context = expect_tuner_service();
    let layout_sig = context.layout;
    let config_sig = context.config;

    let l_slider_ref = NodeRef::new();
    let t_slider_ref = NodeRef::new();
    let r_slider_ref = NodeRef::new();

    let UseElementSizeReturn {
        width: l_slider_width,
        height: l_slider_height,
    } = use_element_size(l_slider_ref);
    let UseElementSizeReturn {
        width: t_slider_width,
        height: t_slider_height,
    } = use_element_size(t_slider_ref);
    let UseElementSizeReturn {
        width: r_slider_width,
        height: r_slider_height,
    } = use_element_size(r_slider_ref);

    let pixel_ratio = use_device_pixel_ratio();

    // World scale in CSS px per world unit (container-sized: 1 CSS px == 1 world unit)
    let world_scale_css = Memo::new(move |_| 1.0);

    // Device scale per world unit
    let device_scale = Memo::new(move |_| world_scale_css() * pixel_ratio());

    // Backing bitmap size in device pixels = layout.space * DPR
    let backing_width_dev = Memo::new(move |_| {
        let pr = pixel_ratio();
        let space_x = layout_sig.get().unwrap_or_default().space.x;
        (space_x * pr).round().clamp(1.0, f64::MAX) as u32
    });

    let backing_height_dev = Memo::new(move |_| {
        let pr = pixel_ratio();
        let space_y = layout_sig.get().unwrap_or_default().space.y;
        (space_y * pr).round().clamp(1.0, f64::MAX) as u32
    });

    let orientation = Signal::derive(move || layout_sig().unwrap_or_default().orientation);
    let orientation_opposite = Signal::derive(move || orientation().opposite());

    let bar_breadth = move |excluded: f64| {
        let c = context.constraints.get();
        let lay = layout_sig.get().unwrap_or_default();
        let (min_lim, max_lim) = c.frequency_limit;
        let full = (max_lim - min_lim).max(0.0) as f64;

        let frac = if full > 0.0 { excluded / full } else { 0.0 };

        let breadth = match orientation() {
            common::orientation::LayoutOrientation::Horizontal => lay.space.x,
            _ => lay.space.y,
        };

        (frac * breadth * OVERLAY_BAR_SCALE_RATIO).max(OVERLAY_BAR_MIN_PX)
    };

    // Pack canonical transform
    let canvas_transform = Memo::new(move |_| {
        // Base parameters
        let pr = pixel_ratio();
        let lay = layout_sig.get().unwrap_or_default();
        let constraints = context.constraints.get();
        let (min_lim, max_lim) = constraints.frequency_limit;

        // Excluded spans (Hz) mapped to fractions of the full spectrum
        let excl_left_hz = constraints
            .min_frequency
            .map(|m| (m - min_lim).max(0.0) as f64)
            .unwrap_or(0.0);
        let excl_right_hz = constraints
            .max_frequency
            .map(|m| (max_lim - m).max(0.0) as f64)
            .unwrap_or(0.0);

        let min_bar_breadth = bar_breadth(excl_left_hz);
        let max_bar_breadth = bar_breadth(excl_right_hz);

        let l_slider_width = l_slider_width();
        let t_slider_width = t_slider_width();
        let r_slider_width = r_slider_width();
        let l_slider_height = l_slider_height();
        let t_slider_height = t_slider_height();
        let r_slider_height = r_slider_height();

        // Margins in CSS px, orientation-aware
        let (margin_left_css, margin_right_css, margin_top_css, margin_bottom_css) =
            match lay.orientation {
                common::orientation::LayoutOrientation::Horizontal => {
                    let left = if constraints.min_frequency.is_some() {
                        min_bar_breadth
                    } else {
                        0.0
                    };
                    let right = if constraints.max_frequency.is_some() {
                        max_bar_breadth
                    } else {
                        0.0
                    };
                    (
                        left,
                        right + t_slider_width + r_slider_width,
                        l_slider_height,
                        0.0,
                    )
                }
                common::orientation::LayoutOrientation::Vertical => {
                    let top = if constraints.min_frequency.is_some() {
                        min_bar_breadth
                    } else {
                        0.0
                    };
                    let bottom = if constraints.max_frequency.is_some() {
                        max_bar_breadth
                    } else {
                        0.0
                    };
                    (
                        0.0,
                        l_slider_width,
                        top + t_slider_height + r_slider_height,
                        bottom,
                    )
                }
            };

        // Base uniform device scale (legacy), plus per-axis safe-area device scales
        let base_device_scale = world_scale_css() * pr;
        let (device_scale_x, device_scale_y) = match lay.orientation {
            common::orientation::LayoutOrientation::Horizontal => {
                let safe_w = (lay.space.x - margin_left_css - margin_right_css).max(1.0);
                (
                    base_device_scale * (safe_w / lay.space.x.max(1.0)),
                    base_device_scale,
                )
            }
            common::orientation::LayoutOrientation::Vertical => {
                let safe_h = (lay.space.y - margin_top_css - margin_bottom_css).max(1.0);
                (
                    base_device_scale,
                    base_device_scale * (safe_h / lay.space.y.max(1.0)),
                )
            }
        };

        // Margin-aware translations in device px (do not override legacy translate_* fields)
        let translate_x_margin_dev = match lay.orientation {
            common::orientation::LayoutOrientation::Horizontal => margin_left_css * pr,
            _ => 0.0,
        };
        let translate_y_margin_dev = match lay.orientation {
            common::orientation::LayoutOrientation::Vertical => margin_top_css * pr,
            _ => 0.0,
        };

        CanvasTransform {
            world_scale_css: world_scale_css(),
            device_scale: device_scale(),
            device_scale_x,
            device_scale_y,
            translate_x_margin_dev,
            translate_y_margin_dev,
            margin_left_css,
            margin_right_css,
            margin_top_css,
            margin_bottom_css,
            backing_width_dev: backing_width_dev(),
            backing_height_dev: backing_height_dev(),
            device_pixel_ratio: pixel_ratio(),
        }
    });

    // Update sensor command
    let UseTauriReturn {
        error: update_sensor_error,
        trigger: update_sensor_invoke,
        data: update_sensor_data,
        ..
    } = use_invoke::<UpdateSensorPayload, (), Config>(common::commands::tuner::UPDATE_SENSOR);

    Effect::new(move |_| {
        if let Some(err) = update_sensor_error() {
            log::error!("Error updating sensor: {err}");
        }
    });

    Effect::new(move |_| {
        if let Some(data) = update_sensor_data() {
            config_sig.set(Some(data));
        }
    });

    let on_update_sensor = Callback::new(move |payload: UpdateSensorPayload| {
        log::debug!("Updating sensor: {:#?}", payload);
        update_sensor_invoke(Some((payload, ())));
    });

    let on_update_range = Callback::new(move |val: SliderValue| {
        let (min, max): (f32, f32) = val.into();

        let ReflectTunerConstraints {
            frequency_limit: (min_lim, max_lim),
            ..
        } = context.constraints.get();
        context.update_spectrum_range.run(UpdateRangePayload {
            min_frequency: if min >= min_lim { Some(min) } else { None },
            max_frequency: if max <= max_lim { Some(max) } else { None },
        })
    });

    let range_value = Signal::derive(move || {
        let ReflectTunerConstraints {
            min_frequency,
            max_frequency,
            frequency_limit,
            ..
        } = context.constraints.get();
        let min = min_frequency.unwrap_or(frequency_limit.0);
        let max = max_frequency.unwrap_or(frequency_limit.1);
        SliderValue::Range(min, max)
    });

    let on_update_ny_threshold = Callback::new(move |val: SliderValue| {
        context.update_ny_threshold.run(val.into());
    });

    let ny_threshold_value =
        Signal::derive(move || SliderValue::Single(context.constraints.get().ny_threshold));

    let on_update_ny_wet_ratio = Callback::new(move |val: SliderValue| {
        context.update_ny_wet_ratio.run(val.into());
    });

    let ny_wet_ratio_value =
        Signal::derive(move || SliderValue::Single(context.constraints.get().wet_ratio));

    // Overlay bar computed styles and visibility
    let show_left_bar = Signal::derive(move || context.constraints.get().min_frequency.is_some());
    let show_right_bar = Signal::derive(move || context.constraints.get().max_frequency.is_some());

    let left_bar_style = Signal::derive(move || {
        let c = context.constraints.get();
        let (min_lim, _) = c.frequency_limit;
        let excluded = c
            .min_frequency
            .map(|m| (m - min_lim).max(0.0) as f64)
            .unwrap_or(0.0);
        let size = bar_breadth(excluded);

        match orientation() {
            common::orientation::LayoutOrientation::Horizontal => format!(
                "left:0;top:0;height:100%;width:{size:.3}px;transition:width {OVERLAY_TRANSITION_MS}ms ease;",
            ),
            _ => format!(
                "left:0;top:0;width:100%;height:{size:.3}px;transition:height {OVERLAY_TRANSITION_MS}ms ease;",
            ),
        }
    });

    let right_bar_style = Signal::derive(move || {
        let c = context.constraints.get();
        let (_, max_lim) = c.frequency_limit;
        let excluded = c
            .max_frequency
            .map(|m| (max_lim - m).max(0.0) as f64)
            .unwrap_or(0.0);
        let size = bar_breadth(excluded);

        match orientation() {
            common::orientation::LayoutOrientation::Horizontal => format!(
                "right:0;top:0;height:100%;width:{size:.3}px;transition:width {OVERLAY_TRANSITION_MS}ms ease;",
            ),
            _ => format!(
                "left:0;bottom:0;width:100%;height:{size:.3}px;transition:height {OVERLAY_TRANSITION_MS}ms ease;",
            ),
        }
    });

    let grid_padding = move || match orientation() {
        common::orientation::LayoutOrientation::Horizontal => {
            format!(
                "0px {}px 0px {}px",
                bar_breadth(
                    context
                        .constraints
                        .get()
                        .min_frequency
                        .map(|m| {
                            (m - context.constraints.get().frequency_limit.0).max(0.0) as f64
                        })
                        .unwrap_or(0.0),
                ),
                bar_breadth(
                    context
                        .constraints
                        .get()
                        .max_frequency
                        .map(|m| {
                            (context.constraints.get().frequency_limit.1 - m).max(0.0) as f64
                        })
                        .unwrap_or(0.0),
                ),
            )
        }
        common::orientation::LayoutOrientation::Vertical => {
            format!(
                "{}px 0px {}px 0px",
                bar_breadth(
                    context
                        .constraints
                        .get()
                        .min_frequency
                        .map(|m| {
                            (m - context.constraints.get().frequency_limit.0).max(0.0) as f64
                        })
                        .unwrap_or(0.0),
                ),
                bar_breadth(
                    context
                        .constraints
                        .get()
                        .max_frequency
                        .map(|m| {
                            (context.constraints.get().frequency_limit.1 - m).max(0.0) as f64
                        })
                        .unwrap_or(0.0),
                ),
            )
        }
    };

    let grid_template = move || match orientation() {
        common::orientation::LayoutOrientation::Vertical => {
            r#"
            "r r l" auto
            "t t l" auto
            ". . l" 1fr
            / 1fr 1fr auto
            "#
        }
        common::orientation::LayoutOrientation::Horizontal => {
            r#"
            "l l l" auto
            ". t r" 1fr
            ". t r" auto
            / 1fr auto auto
            "#
        }
    };

    view! {
        <div
            class="relative overflow-hidden grid justify-items-stretch items-stretch"
            style:width=move || format!("{}px", layout_sig.get().unwrap_or_default().space.x)
            style:height=move || format!("{}px", layout_sig.get().unwrap_or_default().space.y)
            style:padding=grid_padding
            style:grid-template=grid_template
        >
            <SpectrumVisualizer layout=layout_sig canvas_transform=canvas_transform />

            <SensorHandles
                config=Signal::derive(move || config_sig.get().unwrap_or_default())
                layout=Signal::derive(move || layout_sig.get().unwrap_or_default())
                canvas_transform=canvas_transform
                on_update=on_update_sensor
            />

            <Show when=move || show_left_bar()>
                <div
                    class="absolute pointer-events-none backdrop-blur-3xl bg-cinnabar/50 dark:bg-gray/50 shadow-sm"
                    style=left_bar_style
                    role="presentation"
                />
            </Show>
            <Show when=move || show_right_bar()>
                <div
                    class="absolute pointer-events-none backdrop-blur-3xl bg-cinnabar/50 dark:bg-gray/50 shadow-sm"
                    style=right_bar_style
                    role="presentation"
                />
            </Show>

            <RangeSlider
                orientation=orientation
                value=range_value
                on_input=on_update_range
                ui_size=UiSize::Lg
                min=Signal::derive(move || context.constraints.get().frequency_limit.0 - 3.0)
                max=Signal::derive(move || context.constraints.get().frequency_limit.1 + 3.0)
                style:grid-area="l"
                min_label=Signal::derive(move || {
                    format!(
                        "≥ {:.1} Hz",
                        context
                            .constraints
                            .get()
                            .min_frequency
                            .unwrap_or(context.constraints.get().frequency_limit.0),
                    )
                })
                max_label=Signal::derive(move || {
                    format!(
                        "≤ {:.1} Hz",
                        context
                            .constraints
                            .get()
                            .max_frequency
                            .unwrap_or(context.constraints.get().frequency_limit.1),
                    )
                })
                node_ref=l_slider_ref
            />
            <RangeSlider
                orientation=orientation_opposite
                value=ny_threshold_value
                on_input=on_update_ny_threshold
                ui_size=UiSize::Md
                min=0.0
                max=1.0
                style:grid-area="t"
                style:height=move || {
                    if matches!(orientation(), common::orientation::LayoutOrientation::Horizontal) {
                        format!("{}%", 50_f32 + 50_f32 * ny_wet_ratio_value().to_f32())
                    } else {
                        "unset".to_string()
                    }
                }
                style:width=move || {
                    if matches!(orientation(), common::orientation::LayoutOrientation::Vertical) {
                        format!("{}%", 50_f32 + 50_f32 * ny_wet_ratio_value().to_f32())
                    } else {
                        "unset".to_string()
                    }
                }
                label="NY THR"
                node_ref=t_slider_ref
                class="text-center"
            />
            <RangeSlider
                orientation=orientation_opposite
                value=ny_wet_ratio_value
                on_input=on_update_ny_wet_ratio
                ui_size=UiSize::Md
                min=0.0
                max=1.0
                style:grid-area="r"
                label="NY W/D"
                node_ref=r_slider_ref
                class="text-center"
            />
        </div>
    }
}
