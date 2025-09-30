#[allow(unused)]
use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use tauri_use::{use_invoke, use_listen, EventType, UseListenReturn, UseTauriReturn};

/// Options for the combined invoke/event hook
#[derive(Clone)]
pub struct UseInvokeEventOptions {
    /// Whether to fetch data immediately on hook creation
    pub immediate: bool,
    /// Whether to refetch on manual trigger
    pub manual_refetch: bool,
}

impl Default for UseInvokeEventOptions {
    fn default() -> Self {
        Self {
            immediate: true,
            manual_refetch: false,
        }
    }
}

/// Return type for the combined hook
#[derive(Clone)]
#[allow(dead_code)]
pub struct UseTauriResourceReturn<T, F, R>
where
    T: std::fmt::Debug + Clone + 'static,
    F: Fn() + Clone + Send + Sync + 'static,
    R: Fn() + Clone + Send + Sync + 'static,
{
    pub data: ReadSignal<Option<T>>,
    pub loading: ReadSignal<bool>,
    pub error: ReadSignal<Option<String>>,
    pub refetch: F,
    pub reset: R,
}

/// Custom hook that combines invoke and event listening
#[allow(unused)]
pub fn use_tauri_resource_with_args<T, Args, A>(
    common_name: &'static str,
    args: A,
) -> UseTauriResourceReturn<
    T,
    impl Fn() + Clone + Send + Sync + 'static,
    impl Fn() + Clone + Send + Sync + 'static,
>
where
    T: std::fmt::Debug
        + PartialEq
        + Clone
        + for<'de> Deserialize<'de>
        + Serialize
        + Send
        + Sync
        + 'static,
    Args: PartialEq + Serialize + Clone + Send + Sync + 'static,
    A: Into<Signal<Args>>,
{
    use_tauri_resource_with_args_opts(
        common_name,
        args,
        UseInvokeEventOptions {
            immediate: true,
            manual_refetch: true,
        },
    )
}

/// Custom hook that combines invoke and event listening
#[allow(unused)]
pub fn use_tauri_resource<T>(
    common_name: &'static str,
) -> UseTauriResourceReturn<
    T,
    impl Fn() + Clone + Send + Sync + 'static,
    impl Fn() + Clone + Send + Sync + 'static,
>
where
    T: std::fmt::Debug
        + PartialEq
        + Clone
        + for<'de> Deserialize<'de>
        + Serialize
        + Send
        + Sync
        + 'static,
{
    // No-args convenience: provide a unit Signal so generic inference succeeds.
    use_tauri_resource_with_args_opts::<T, (), Signal<()>>(
        common_name,
        Signal::derive(|| ()),
        UseInvokeEventOptions {
            immediate: true,
            manual_refetch: true,
        },
    )
}

/// Custom hook that combines invoke and event listening
#[allow(unused)]
pub fn use_tauri_resource_with_opts<T>(
    common_name: &'static str,
    options: UseInvokeEventOptions,
) -> UseTauriResourceReturn<
    T,
    impl Fn() + Clone + Send + Sync + 'static,
    impl Fn() + Clone + Send + Sync + 'static,
>
where
    T: std::fmt::Debug
        + PartialEq
        + Clone
        + for<'de> Deserialize<'de>
        + Serialize
        + Send
        + Sync
        + 'static,
{
    // No-args + custom options variant.
    use_tauri_resource_with_args_opts::<T, (), Signal<()>>(
        common_name,
        Signal::derive(|| ()),
        options,
    )
}

/// Custom hook that combines invoke and event listening
#[allow(unused)]
pub fn use_tauri_resource_with_args_opts<T, Args, A>(
    common_name: &'static str,
    args: A,
    options: UseInvokeEventOptions,
) -> UseTauriResourceReturn<
    T,
    impl Fn() + Clone + Send + Sync + 'static,
    impl Fn() + Clone + Send + Sync + 'static,
>
where
    T: std::fmt::Debug
        + PartialEq
        + Clone
        + for<'de> Deserialize<'de>
        + Serialize
        + Send
        + Sync
        + 'static,
    Args: PartialEq + Serialize + Clone + Send + Sync + 'static,
    A: Into<Signal<Args>>,
{
    let args: Signal<Args> = args.into();

    #[derive(Clone, PartialEq)]
    struct TriggerData<A> {
        args: A,
        count: usize,
    }

    // Internal state
    let (data, set_data) = signal::<Option<T>>(None);
    let (loading, set_loading) = signal(false);
    let (error, set_error) = signal::<Option<String>>(None);
    let (inner_trigger, set_inner_trigger) = signal(TriggerData::<Args> {
        args: args.get_untracked(),
        count: 0,
    });

    // Helper function to update data and clear loading/error states
    let update_data = move |new_data: T| {
        set_data.set(Some(new_data));
        set_loading.set(false);
        set_error.set(None);
    };

    // Helper function to handle errors
    let handle_error = move |error_msg: String| {
        set_error.set(Some(error_msg));
        set_loading.set(false);
    };

    // Initial fetch using tauri_use
    let UseTauriReturn {
        data: invoke_data,
        error: invoke_error,
        trigger: invoke_trigger,
    } = use_invoke::<Args, (), T>(common_name);

    // Listen to events with the same name as the command
    let UseListenReturn {
        data: event_data,
        error: event_error,
        open: event_open,
        close: event_close,
        ..
    } = use_listen::<T>(EventType::Custom(common_name));

    // Manual refetch function
    let refetch = move || {
        if options.manual_refetch {
            set_loading.set(true);
            set_error.set(None);
            set_inner_trigger.update(|n| n.count += 1);
        }
    };

    // Reset function
    let reset = move || {
        set_data.set(None);
        set_loading.set(false);
        set_error.set(None);
    };

    Effect::new(move || {
        let data_value = invoke_data();
        let event_data = event_data();

        if let Some(data) = event_data {
            log::debug!("Event data received for `{common_name}`: {data:?}");
            update_data(data);
        } else if let Some(data) = data_value {
            log::debug!("Invoke data received for `{common_name}`: {data:?}");
            update_data(data);
        }
    });

    // Track loading state
    Effect::new(move || {
        let is_loading = invoke_data().is_none() && inner_trigger().count > 0;
        set_loading.set(is_loading);
    });

    Effect::new(move || {
        event_open();
        if options.immediate {
            set_inner_trigger.update(|n| n.count = 1);
        }
    });

    Effect::new(move || {
        let TriggerData { args, count } = inner_trigger();
        if count > 0 {
            invoke_trigger(Some((args, ())))
        }
    });

    Effect::new(move || {
        if let Some(err) = invoke_error() {
            log::error!("invoke error: {err}");
            handle_error(err.to_string());
        }
        if let Some(err) = event_error() {
            log::error!("event error: {err}");
            handle_error(err.to_string());
        }
    });

    on_cleanup(move || {
        event_close();
    });

    UseTauriResourceReturn {
        data,
        loading,
        error,
        refetch,
        reset,
    }
}
