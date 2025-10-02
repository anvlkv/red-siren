use leptos::prelude::*;
use leptos_use::{utils::Pausable, UseRafFnOptions};
use std::cell::Cell;
use std::rc::Rc;

pub use leptos_use::UseRafFnCallbackArgs;

/// Simple RAF function with fixed FPS throttling
pub fn use_raf_fn_with_fps<F>(
    callback: F,
    fps: f64,
) -> Pausable<impl Fn() + Clone + Send + Sync, impl Fn() + Clone + Send + Sync>
where
    F: Fn(UseRafFnCallbackArgs) + 'static,
{
    use_raf_fn_with_fps_and_options(callback, fps, UseRafFnWithFpsOptions::default())
}

/// Fixed-fps with options
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

/// Signal-driven FPS version - reactive to FPS changes
pub fn use_raf_fn_with_fps_signal<F>(
    callback: F,
    fps: Signal<f64>,
) -> Pausable<impl Fn() + Clone + Send + Sync, impl Fn() + Clone + Send + Sync>
where
    F: Fn(UseRafFnCallbackArgs) + 'static,
{
    throttled_runner(callback, fps, UseRafFnWithFpsOptions::default())
}

/// Core throttled runner with simplified timing
fn throttled_runner<F>(
    callback: F,
    fps: Signal<f64>,
    options: UseRafFnWithFpsOptions,
) -> Pausable<impl Fn() + Clone + Send + Sync, impl Fn() + Clone + Send + Sync>
where
    F: Fn(UseRafFnCallbackArgs) + 'static,
{
    let last_frame_time = Rc::new(Cell::new(0.0));
    let accumulated_time = Rc::new(Cell::new(0.0));
    let current_fps_interval = Rc::new(Cell::new(0.0));

    // Update FPS interval when signal changes
    {
        let current_fps_interval = Rc::clone(&current_fps_interval);
        let last_frame_time = Rc::clone(&last_frame_time);
        Effect::new(move |_| {
            let f = fps().max(1.0);
            let interval = 1000.0 / f;
            current_fps_interval.set(interval);
            // Reset on FPS change to avoid jumps
            last_frame_time.set(0.0);
        });
    }

    let throttled_callback = {
        let last_frame_time = Rc::clone(&last_frame_time);
        let accumulated_time = Rc::clone(&accumulated_time);
        let current_fps_interval = Rc::clone(&current_fps_interval);

        move |args: UseRafFnCallbackArgs| {
            let current_time = args.timestamp;
            let last_time = last_frame_time.get();
            let fps_interval = current_fps_interval.get();

            if last_time == 0.0 {
                // First frame after reset
                last_frame_time.set(current_time);
                accumulated_time.set(0.0);
                callback(UseRafFnCallbackArgs {
                    delta: 0.0,
                    timestamp: current_time,
                });
            } else {
                // Accumulate time since last frame
                let time_since_last = accumulated_time.get() + (current_time - last_time);
                accumulated_time.set(time_since_last);
                last_frame_time.set(current_time);

                // Execute callback if we've accumulated enough time for a frame
                if time_since_last >= fps_interval {
                    // Consume one frame's worth of time
                    accumulated_time.set(time_since_last - fps_interval);

                    // Use the actual delta between executions
                    callback(UseRafFnCallbackArgs {
                        delta: fps_interval,
                        timestamp: current_time,
                    });
                }
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
