use leptos::prelude::*;
use serde::de::DeserializeOwned;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::spawn_local;
use log::error;


use super::tauri_channel::{invoke, TypedChannel};

/// Create a typed Tauri Channel and a reactive Signal holding the latest message.
///
/// Returns (latest_signal, channel_js_value).
/// - `latest_signal`: updates to `Some(T)` on each message; starts as `None`.
/// - `channel_js_value`: pass this under some key (e.g. `onEvent`) when invoking a command.
///
/// The channel is *receiving only*; sending from the frontend is not supported by Tauri Channels.
pub fn use_typed_channel<T>() -> (RwSignal<Option<T>>, TypedChannel<T>)
where
    T: DeserializeOwned + Clone + Sync + Send + 'static,
{
    let latest: RwSignal<Option<T>> = RwSignal::new(None);
    let channel = TypedChannel::<T>::new();

    channel.set_onmessage(move |msg| {
        latest.set(Some(msg));
    });

    (latest, channel)
}

/// Invoke a backend command passing the channel under `channel_key` plus any extra serializable args.
///
/// - `cmd`: Tauri command string
/// - `extra_args`: anything `serde::Serialize`
/// - `channel_key`: the object key under which the channel will be attached (e.g. `"onEvent"`)
/// - `channel_js`: the JsValue from `use_typed_channel`
///
/// Returns immediately; spawns an async task to perform the invoke call.
/// Logs errors to the browser console.
///
/// Type Parameters:
/// - `A`: extra args type (Serialize)
/// - `R`: expected deserialize type from backend response (use `()` if no response needed)
pub fn invoke_with_channel<A, R>(cmd: &str, extra_args: &A, channel_key: &str, channel_js: JsValue)
where
    A: serde::Serialize + ?Sized,
    R: DeserializeOwned + 'static,
{
    // Build a merged JSON object: { channel_key: channel_js, ...extra_args }
    // We serialize extra_args first, then inject the channel.
    let args_js = match serde_wasm_bindgen::to_value(extra_args) {
        Ok(v) => v,
        Err(err) => {
            error!("invoke_with_channel: serialize extra_args failed: {err}");
            return;
        }
    };

    // We want an object; if not, wrap into one.
    let obj = if args_js.is_object() {
        args_js
    } else {
        let map = js_sys::Object::new();
        map.into()
    };

    // Set the channel field (obj guaranteed object above)
    let _ = js_sys::Reflect::set(&obj, &JsValue::from_str(channel_key), &channel_js);

    let cmd_string = cmd.to_string();
    spawn_local(async move {
        // New invoke signature returns JsValue directly (throws on JS-side error).
        let result_js = invoke(&cmd_string, obj).await;
        if std::any::TypeId::of::<R>() != std::any::TypeId::of::<()>() {
            if let Ok(parsed) = serde_wasm_bindgen::from_value::<R>(result_js.clone()) {
                let _ = parsed;
            }
        }
    });
}

/// Convenience: invoke a command that only needs the channel (no extra args).
///
/// Equivalent to:
/// `invoke_with_channel::<(), ()>(cmd, &(), channel_key, channel_js)`
pub fn invoke_channel_only(cmd: &str, channel_key: &str, channel_js: JsValue) {
    invoke_with_channel::<(), ()>(cmd, &(), channel_key, channel_js);
}
