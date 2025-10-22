use leptos::prelude::*;
use leptos_use::{utils::Pausable, UseRafFnOptions};

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

/// Core throttled runner with simplified timing
fn throttled_runner<F>(
    callback: F,
    fps: Signal<f64>,
    options: UseRafFnWithFpsOptions,
) -> Pausable<impl Fn() + Clone + Send + Sync, impl Fn() + Clone + Send + Sync>
where
    F: Fn(UseRafFnCallbackArgs) + 'static,
{
    let last_frame_time = StoredValue::new(0.0);
    let last_execution_time = StoredValue::new(0.0);
    let accumulated_time = StoredValue::new(0.0);
    let current_fps_interval = StoredValue::new(0.0);

    // Update FPS interval when signal changes
    {
        Effect::new(move |_| {
            let f = fps().max(1.0);
            let interval = 1000.0 / f;
            current_fps_interval.set_value(interval);
            // Reset on FPS change to avoid jumps
            last_frame_time.set_value(0.0);
            last_execution_time.set_value(0.0);
        });
    }

    let throttled_callback = {
        move |args: UseRafFnCallbackArgs| {
            let current_time = args.timestamp;
            let last_time = last_frame_time.get_value();
            let fps_interval = current_fps_interval.get_value();

            if last_time == 0.0 {
                // First frame after reset
                last_frame_time.set_value(current_time);
                last_execution_time.set_value(current_time);
                accumulated_time.set_value(0.0);
                callback(UseRafFnCallbackArgs {
                    delta: 0.0,
                    timestamp: current_time,
                });
            } else {
                // Accumulate time since last frame
                let time_since_last = accumulated_time.get_value() + (current_time - last_time);
                accumulated_time.set_value(time_since_last);
                last_frame_time.set_value(current_time);

                // Execute callback if we've accumulated enough time for a frame
                if time_since_last >= fps_interval {
                    // Reset accumulated time to prevent catch-up overruns
                    accumulated_time.set_value(0.0);

                    // Calculate actual delta since last execution
                    let last_exec = last_execution_time.get_value();
                    let actual_delta = if last_exec == 0.0 {
                        fps_interval
                    } else {
                        current_time - last_exec
                    };
                    last_execution_time.set_value(current_time);

                    callback(UseRafFnCallbackArgs {
                        delta: actual_delta,
                        timestamp: current_time,
                    });
                }
            }
        }
    };

    let raf_fn = leptos_use::use_raf_fn_with_options(
        throttled_callback,
        UseRafFnOptions::default().immediate(options.immediate),
    );

    // Wrap pause/resume to reset timing state
    let resume = {
        let original_resume = raf_fn.resume.clone();

        move || {
            // Reset timing state on resume
            last_frame_time.set_value(0.0);
            last_execution_time.set_value(0.0);
            accumulated_time.set_value(0.0);
            original_resume();
        }
    };

    let pause = {
        let original_pause = raf_fn.pause.clone();

        move || {
            original_pause();
            // Reset timing state on pause
            last_frame_time.set_value(0.0);
            last_execution_time.set_value(0.0);
            accumulated_time.set_value(0.0);
        }
    };

    Pausable {
        pause,
        resume,
        is_active: raf_fn.is_active,
    }
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
