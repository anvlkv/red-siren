/*!
Generic typed Tauri Channel utilities for the frontend (Leptos + WASM).

PURPOSE
Minimal, reusable abstraction to:
1. Create a type-safe Tauri Channel receiving streaming events.
2. Expose a reactive `Signal<Option<T>>` updated on each incoming message.
3. Provide helpers to invoke backend commands that expect `{ onEvent: Channel, ... }`.

MAYA DRY KISS
Keep the API small; extend only when concrete new patterns emerge.

USAGE EXAMPLE
-----------
```ignore
use crate::utils::channel::{use_typed_channel, invoke_with_channel};
use shared::commands::intro::{INTRO_STREAM, INTRO_PAUSE, INTRO_RESUME};
use shared::events::intro::IntroSnoopBatchPayload;
use leptos::prelude::*;
use serde_json::json;

#[component]
fn IntroWaves() -> impl IntoView {
    let (latest_batch, channel_js) = use_typed_channel::<IntroSnoopBatchPayload>();

    // Start stream on mount
    Effect::new(move |_| {
        // Pass only the channel (payload is () on the Rust side)
        invoke_with_channel::<(), ()>(
            INTRO_STREAM,
            &json!({}),         // extra args (none here)
            "onEvent",
            channel_js.clone(),
        );
    });

    view! {
        <div>
            {move || latest_batch.get().map(|b| format!("Got {} snoops", b.snoops.len()))}
        </div>
    }
}
```

NOTES
-----
- This does not throttle or buffer; the last event replaces the previous.
- If you need a history, wrap the signal and push into a Vec inside an `Effect`.
- Errors during invoke are logged to the console (non-fatal).
*/

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
pub fn use_typed_channel<T>() -> (RwSignal<Option<T>>, JsValue)
where
    T: DeserializeOwned + Clone + Sync + Send + 'static,
{
    let latest: RwSignal<Option<T>> = RwSignal::new(None);
    let channel = TypedChannel::<T>::new();

    channel.set_onmessage(move |msg| {
        latest.set(Some(msg));
    });

    (latest, channel.as_jsvalue())
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

/// Build SVG polyline/path data (M/L format) from evenly spaced samples.
///
/// Utility for wave rendering:
/// - `samples`: sequence of y values (-1..1 expected but not enforced)
/// - `length_px`: total horizontal length
/// - `center_x`: horizontal center coordinate
/// - `center_y`: baseline y coordinate
/// - `amplitude_px`: vertical scaling factor
///
/// Returns an SVG path string `M x0 y0 L x1 y1 ...`.
pub fn waveform_path(
    samples: &[f32],
    length_px: f32,
    center_x: f32,
    center_y: f32,
    amplitude_px: f32,
) -> String {
    if samples.is_empty() {
        return String::new();
    }
    if samples.len() == 1 {
        return format!(
            "M{:.2} {:.2}L{:.2} {:.2}",
            center_x,
            center_y,
            center_x,
            center_y - samples[0] * amplitude_px
        );
    }

    let dx = length_px / (samples.len() - 1) as f32;
    let start_x = center_x - length_px * 0.5;

    let mut s = String::with_capacity(samples.len() * 12);
    let y0 = center_y - samples[0] * amplitude_px;
    s.push_str(&format!("M{:.2} {:.2}", start_x, y0));

    for (i, sample) in samples.iter().enumerate().skip(1) {
        let x = start_x + dx * i as f32;
        let y = center_y - sample * amplitude_px;
        // Avoid trailing space/newline to keep string compact
        s.push_str(&format!("L{:.2} {:.2}", x, y));
    }
    s
}
