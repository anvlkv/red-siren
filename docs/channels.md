## WASM Bindings for Tauri Channel API

```rust
// src/bindings/tauri_channel.rs
use wasm_bindgen::prelude::*;
use js_sys::Function;
use serde::{Deserialize, Serialize};

#[wasm_bindgen]
extern "C" {
    // Channel type binding
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "channel"])]
    type Channel;

    #[wasm_bindgen(constructor, js_namespace = ["window", "__TAURI__", "channel"])]
    fn new() -> Channel;

    #[wasm_bindgen(method, setter, js_name = onmessage)]
    fn set_onmessage(this: &Channel, callback: &Function);

    #[wasm_bindgen(method, getter, js_name = onmessage)]
    fn onmessage(this: &Channel) -> Option<Function>;

    // Core invoke function
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"])]
    async fn invoke(cmd: &str, args: JsValue) -> Result<JsValue, JsValue>;
}

// Re-export Channel for easier use
#[wasm_bindgen]
extern "C" {
    #[derive(Clone, Debug)]
    pub type TauriChannel;
}
```

## Type-safe Channel Implementation

```rust
// src/utils/channel.rs
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use js_sys::Function;
use serde::{Deserialize, Serialize};

pub struct TypedChannel<T>
where
    T: for<'de> Deserialize<'de> + Clone + 'static,
{
    inner: Channel,
    _phantom: std::marker::PhantomData<T>,
}

impl<T> TypedChannel<T>
where
    T: for<'de> Deserialize<'de> + Clone + 'static,
{
    pub fn new() -> Self {
        Self {
            inner: Channel::new(),
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn set_onmessage<F>(&self, callback: F)
    where
        F: Fn(T) + 'static,
    {
        let closure = Closure::wrap(Box::new(move |event: JsValue| {
            match serde_wasm_bindgen::from_value::<T>(event) {
                Ok(message) => callback(message),
                Err(err) => {
                    web_sys::console::error_1(&format!("Failed to deserialize channel message: {}", err).into());
                }
            }
        }) as Box<dyn Fn(JsValue)>);

        self.inner.set_onmessage(closure.as_ref().unchecked_ref());
        closure.forget(); // Keep closure alive
    }

    pub fn as_jsvalue(&self) -> JsValue {
        self.inner.clone().into()
    }
}
```

## Download Event Types

```rust
// src/types/download.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "event", content = "data")]
pub enum DownloadEvent {
    #[serde(rename = "started")]
    Started {
        url: String,
        #[serde(rename = "downloadId")]
        download_id: u32,
        #[serde(rename = "contentLength")]
        content_length: u64,
    },
    #[serde(rename = "progress")]
    Progress {
        #[serde(rename = "downloadId")]
        download_id: u32,
        #[serde(rename = "chunkLength")]
        chunk_length: u32,
    },
    #[serde(rename = "finished")]
    Finished {
        #[serde(rename = "downloadId")]
        download_id: u32,
    },
}
```

## Leptos Hook Implementation

```rust
// src/hooks/use_download.rs
use leptos::prelude::*;
use crate::utils::channel::TypedChannel;
use crate::types::download::DownloadEvent;
use crate::bindings::tauri_channel::invoke;
use serde_json::json;

pub fn use_download() -> (
    Signal<Option<DownloadEvent>>,  // Latest event
    Signal<Vec<DownloadEvent>>,     // Event history
    Signal<Option<String>>,         // Error
    WriteSignal<Option<String>>,    // URL trigger
) {
    let (latest_event, set_latest_event) = signal(None::<DownloadEvent>);
    let (event_history, set_event_history) = signal(Vec::<DownloadEvent>::new());
    let (error, set_error) = signal(None::<String>);
    let (url_trigger, set_url_trigger) = signal(None::<String>);

    Effect::new(move |_| {
        if let Some(url) = url_trigger.get() {
            spawn_local(async move {
                // Reset state
                set_latest_event.set(None);
                set_event_history.set(Vec::new());
                set_error.set(None);

                // Create typed channel
                let channel = TypedChannel::<DownloadEvent>::new();

                // Set up message handler
                channel.set_onmessage({
                    let set_latest_event = set_latest_event.clone();
                    let set_event_history = set_event_history.clone();

                    move |event: DownloadEvent| {
                        leptos::logging::log!("Got download event: {:?}", event);

                        // Update latest event
                        set_latest_event.set(Some(event.clone()));

                        // Add to history
                        set_event_history.update(|history| {
                            history.push(event);
                        });
                    }
                });

                // Prepare arguments
                let args = json!({
                    "url": url,
                    "onEvent": channel.as_jsvalue()
                });

                let args_value = match serde_wasm_bindgen::to_value(&args) {
                    Ok(val) => val,
                    Err(err) => {
                        set_error.set(Some(format!("Serialization error: {}", err)));
                        return;
                    }
                };

                // Invoke download command
                match invoke("download", args_value).await {
                    Ok(_) => {
                        leptos::logging::log!("Download command completed");
                    }
                    Err(err) => {
                        let error_msg = err.as_string().unwrap_or_else(|| "Unknown error".to_string());
                        set_error.set(Some(error_msg));
                    }
                }
            });

            // Reset trigger
            set_url_trigger.set(None);
        }
    });

    (
        latest_event.into(),
        event_history.into(),
        error.into(),
        set_url_trigger,
    )
}
```

## Component Usage

```rust
// src/components/download_manager.rs
use leptos::prelude::*;
use crate::hooks::use_download::*;
use crate::types::download::DownloadEvent;

#[component]
pub fn DownloadManager() -> impl IntoView {
    let (latest_event, event_history, error, start_download) = use_download();
    let (url, set_url) = signal(String::from(
        "https://raw.githubusercontent.com/tauri-apps/tauri/dev/crates/tauri-schema-generator/schemas/config.schema.json"
    ));

    // Download progress state
    let (download_progress, set_download_progress) = signal(None::<(u32, u64, u64)>); // (id, downloaded, total)
    let (is_downloading, set_is_downloading) = signal(false);

    // Process events for UI state
    Effect::new(move |_| {
        if let Some(event) = latest_event.get() {
            match event {
                DownloadEvent::Started { download_id, content_length, .. } => {
                    set_is_downloading.set(true);
                    set_download_progress.set(Some((download_id, 0, content_length)));
                }
                DownloadEvent::Progress { download_id, chunk_length } => {
                    set_download_progress.update(|progress| {
                        if let Some((id, downloaded, total)) = progress {
                            if *id == download_id {
                                *downloaded += chunk_length as u64;
                            }
                        }
                    });
                }
                DownloadEvent::Finished { .. } => {
                    set_is_downloading.set(false);
                }
            }
        }
    });

    let start_download_click = move |_| {
        if !url.get().is_empty() && !is_downloading.get() {
            start_download.set(Some(url.get()));
        }
    };

    view! {
        <div class="download-manager">
            <h2>"Tauri Channel Download Manager"</h2>

            <div class="input-group">
                <input
                    type="url"
                    placeholder="Enter download URL"
                    prop:value=url
                    on:input=move |ev| {
                        set_url.set(event_target_value(&ev));
                    }
                    disabled=is_downloading
                />
                <button
                    on:click=start_download_click
                    disabled=move || is_downloading.get() || url.get().is_empty()
                >
                    {move || if is_downloading.get() { "Downloading..." } else { "Start Download" }}
                </button>
            </div>

            // Progress display
            {move || {
                download_progress.get().map(|(id, downloaded, total)| {
                    let percentage = if total > 0 {
                        (downloaded as f64 / total as f64 * 100.0) as u32
                    } else {
                        0
                    };

                    view! {
                        <div class="progress-section">
                            <h3>"Download Progress"</h3>
                            <div class="progress-bar">
                                <div
                                    class="progress-fill"
                                    style:width=format!("{}%", percentage)
                                ></div>
                            </div>
                            <div class="progress-stats">
                                <span>"ID: " {id}</span>
                                <span>{format!("{:.1} KB / {:.1} KB", downloaded as f64 / 1024.0, total as f64 / 1024.0)}</span>
                                <span>{percentage}"%"</span>
                            </div>
                        </div>
                    }
                })
            }}

            // Latest event display
            {move || {
                latest_event.get().map(|event| view! {
                    <div class="latest-event">
                        <h3>"Latest Event"</h3>
                        <pre>{format!("{:#?}", event)}</pre>
                    </div>
                })
            }}

            // Event history
            <div class="event-history">
                <h3>"Event History"</h3>
                <div class="events-list">
                    {move || {
                        event_history.get().into_iter().enumerate().map(|(i, event)| {
                            view! {
                                <div class="event-item" key=i>
                                    <div class="event-type">
                                        {match &event {
                                            DownloadEvent::Started { .. } => "🟢 Started",
                                            DownloadEvent::Progress { .. } => "🔄 Progress",
                                            DownloadEvent::Finished { .. } => "✅ Finished",
                                        }}
                                    </div>
                                    <div class="event-details">
                                        {match event {
                                            DownloadEvent::Started { url, download_id, content_length } => {
                                                view! { <span>"ID: " {download_id} " | Size: " {format!("{:.1} KB", content_length as f64 / 1024.0)}</span> }
                                            }
                                            DownloadEvent::Progress { download_id, chunk_length } => {
                                                view! { <span>"ID: " {download_id} " | Chunk: " {format!("{} bytes", chunk_length)}</span> }
                                            }
                                            DownloadEvent::Finished { download_id } => {
                                                view! { <span>"ID: " {download_id} " | Complete"</span> }
                                            }
                                        }}
                                    </div>
                                </div>
                            }
                        }).collect_view()
                    }}
                </div>
            </div>

            // Error display
            {move || {
                error.get().map(|err| view! {
                    <div class="error-section">
                        <h3>"Error"</h3>
                        <div class="error-message">{err}</div>
                    </div>
                })
            }}
        </div>
    }
}
```


This implementation provides a complete, type-safe wrapper around Tauri's Channel API that closely mirrors the JavaScript example you provided. The WASM bindings directly interface with Tauri's channel system, and the Leptos components provide reactive UI updates based on the streaming download events.
