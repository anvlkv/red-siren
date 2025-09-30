use leptos_use::utils::Pausable;
use std::cell::Cell;
use std::rc::Rc;

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

pub fn use_raf_fn_with_fps_and_options<F>(
    callback: F,
    fps: f64,
    options: UseRafFnWithFpsOptions,
) -> Pausable<impl Fn() + Clone + Send + Sync, impl Fn() + Clone + Send + Sync>
where
    F: Fn(UseRafFnCallbackArgs) + 'static,
{
    let fps_interval = 1000.0 / fps.max(1.0); // Ensure minimum 1 FPS
    let last_execution_time = Rc::new(Cell::new(0.0));

    let throttled_callback = {
        let last_execution_time = Rc::clone(&last_execution_time);
        move |args: leptos_use::UseRafFnCallbackArgs| {
            let current_time = args.timestamp;
            let last_time = last_execution_time.get();

            // Check if enough time has passed for the desired FPS
            if current_time - last_time >= fps_interval {
                // Calculate accurate delta for the throttled callback
                let throttled_delta = if last_time > 0.0 {
                    current_time - last_time
                } else {
                    0.0
                };

                // Update the last execution time, accounting for frame timing precision
                last_execution_time.set(current_time - ((current_time - last_time) % fps_interval));

                // Call the user's callback with adjusted timing
                callback(UseRafFnCallbackArgs {
                    delta: throttled_delta,
                    timestamp: current_time,
                });
            }
        }
    };

    // Use the standard use_raf_fn with our throttled callback
    leptos_use::use_raf_fn_with_options(
        throttled_callback,
        leptos_use::UseRafFnOptions::default().immediate(options.immediate),
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

// Re-export the callback args type for convenience
pub use leptos_use::UseRafFnCallbackArgs;
