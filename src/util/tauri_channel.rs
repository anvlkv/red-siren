use wasm_bindgen::prelude::*;
use js_sys::Function;
use log::error;

/// WASM bindings for the Tauri 2 Channel API.
///
/// These bindings mirror the JavaScript usage:
/// ```js
/// import { Channel, invoke } from "@tauri-apps/api/core";
///
/// const onEvent = new Channel();
/// onEvent.onmessage = (payload) => { /* handle */ };
/// await invoke("intro_stream", { onEvent });
/// ```
///
/// In Rust (wasm):
/// ```ignore
/// let channel = TauriChannel::new();
/// channel.set_onmessage(|value: JsValue| { /* ... */ });
/// invoke("intro_stream", args_js_value).await?;
/// ```
#[wasm_bindgen]
extern "C" {
    // Channel + invoke live under __TAURI__.core in Tauri 2
    #[wasm_bindgen(js_namespace = ["__TAURI__", "core"], js_name = Channel)]
    type Channel;

    #[wasm_bindgen(constructor, js_namespace = ["__TAURI__", "core"], js_class = Channel)]
    fn new() -> Channel;

    #[wasm_bindgen(method, setter, js_name = onmessage)]
    fn set_onmessage(this: &Channel, callback: &Function);

    #[wasm_bindgen(method, getter, js_name = onmessage)]
    fn onmessage(this: &Channel) -> Option<Function>;

    #[wasm_bindgen(js_namespace = ["__TAURI__", "core"])]
    pub async fn invoke(cmd: &str, args: JsValue) -> JsValue;
}

/// Public newtype wrapper exported with a friendlier name.
/// This allows cloning and debug printing.
#[wasm_bindgen]
extern "C" {
    #[derive(Clone, Debug)]
    pub type TauriChannel;
}

// (Removed redundant From<Channel> for JsValue implementation; wasm-bindgen already provides this.)

/// Typed convenience wrapper providing ergonomic Rust closures.
pub struct TypedChannel<T>
where
    T: for<'de> serde::Deserialize<'de> + Clone + 'static,
{
    inner: Channel,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> TypedChannel<T>
where
    T: for<'de> serde::Deserialize<'de> + Clone + 'static,
{
    pub fn new() -> Self {
        Self {
            inner: Channel::new(),
            _phantom: std::marker::PhantomData,
        }
    }

    /// Attach a strongly typed message handler.
    pub fn set_onmessage<F>(&self, handler: F)
    where
        F: Fn(T) + 'static,
    {
        use wasm_bindgen::JsCast;

        let closure = Closure::wrap(Box::new(move |value: JsValue| {
            match serde_wasm_bindgen::from_value::<T>(value) {
                Ok(v) => handler(v),
                Err(err) => {
                    error!("TypedChannel deserialize error: {err}");
                }
            }
        }) as Box<dyn Fn(JsValue)>);

        self.inner.set_onmessage(closure.as_ref().unchecked_ref());
        closure.forget(); // leak to keep alive
    }

    /// Convert to a JS value for passing into `invoke` argument objects.
    pub fn as_jsvalue(&self) -> JsValue {
        self.inner.clone().into()
    }
}
