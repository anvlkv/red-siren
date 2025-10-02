use leptos::prelude::*;
use leptos_use::{utils::Pausable, UseRafFnOptions};
use std::cell::Cell;
use std::rc::Rc;

pub use leptos_use::UseRafFnCallbackArgs;

#[allow(dead_code)]
pub fn use_raf_fn_with_fps<F>(
    callback: F,
    fps: f64,
) -> Pausable<impl Fn() + Clone + Send + Sync, impl Fn() + Clone + Send + Sync>
where
    F: Fn(UseRafFnCallbackArgs) + 'static,
{
    use_raf_fn_with_fps_and_options(callback, fps, UseRafFnWithFpsOptions::default())
}

/// Fixed-fps with options (legacy)
pub fn use_raf_fn_with_fps_and_options<F>(
    callback: F,
    fps: f64,
    options: UseRafFnWithFpsOptions,
) -> Pausable<impl Fn() + Clone + Send + Sync, impl Fn() + Clone + Send + Sync>
where
    F: Fn(UseRafFnCallbackArgs) + 'static,
{
    throttled_runner(callback, Signal::derive(move || fps), options)
}

/// NEW: Dynamic FPS version controlled by a reactive Signal<f64>.
/// Whenever the fps signal changes, the internal accumulator resets to avoid stutter.
pub fn use_raf_fn_with_fps_signal<F>(
    callback: F,
    fps: Signal<f64>,
) -> Pausable<impl Fn() + Clone + Send + Sync, impl Fn() + Clone + Send + Sync>
where
    F: Fn(UseRafFnCallbackArgs) + 'static,
{
    throttled_runner(callback, fps, UseRafFnWithFpsOptions::default())
}

/// Core throttled runner used by both fixed and signal-driven APIs.
fn throttled_runner<F>(
    callback: F,
    fps: Signal<f64>,
    options: UseRafFnWithFpsOptions,
) -> Pausable<impl Fn() + Clone + Send + Sync, impl Fn() + Clone + Send + Sync>
where
    F: Fn(UseRafFnCallbackArgs) + 'static,
{
    let last_execution_time = Rc::new(Cell::new(0.0));
    let last_fps_interval = Rc::new(Cell::new(0.0));
    let last_fps_value = Rc::new(Cell::new(0.0));

    // Reactive effect: update stored fps interval when fps signal changes
    {
        let last_fps_interval = Rc::clone(&last_fps_interval);
        let last_fps_value = Rc::clone(&last_fps_value);
        let last_execution_time_effect = Rc::clone(&last_execution_time);
        Effect::new(move |_| {
            let f = fps().max(1.0);
            let interval = 1000.0 / f;
            last_fps_interval.set(interval);
            last_fps_value.set(f);
            // Reset timing so first frame after change is immediate
            last_execution_time_effect.set(0.0);
        });
    }

    let throttled_callback = {
        let last_execution_time_cb = Rc::clone(&last_execution_time);
        let last_fps_interval = Rc::clone(&last_fps_interval);
        move |args: UseRafFnCallbackArgs| {
            let current_time = args.timestamp;
            let last_time = last_execution_time_cb.get();
            let fps_interval = last_fps_interval.get().max(1.0);

            if last_time == 0.0 || current_time - last_time >= fps_interval {
                let throttled_delta = if last_time > 0.0 {
                    current_time - last_time
                } else {
                    0.0
                };
                // Snap execution time to an aligned boundary to reduce jitter accumulation
                let aligned_time = if last_time == 0.0 {
                    current_time
                } else {
                    current_time - ((current_time - last_time) % fps_interval)
                };
                last_execution_time_cb.set(aligned_time);
                callback(UseRafFnCallbackArgs {
                    delta: throttled_delta,
                    timestamp: current_time,
                });
            }
        }
    };

    leptos_use::use_raf_fn_with_options(
        throttled_callback,
        UseRafFnOptions::default().immediate(options.immediate),
    )
}

#[derive(Clone)]
pub struct UseRafFnWithFpsOptions {
    pub immediate: bool,
}

impl Default for UseRafFnWithFpsOptions {
    fn default() -> Self {
        Self { immediate: true }
    }
}
