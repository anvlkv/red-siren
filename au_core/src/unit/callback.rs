async fn run_audio_ctx(
    app_au_buffer: AppAuBuffer,
) -> Result<(
    web_sys::AudioWorkletNode,
    web_sys::AudioContext,
    WasmClosure,
)> {
    use js_sys::{Function, Object, Promise, Reflect, Uint8Array};
    use wasm_bindgen::{closure::Closure, JsCast, JsValue};
    use wasm_bindgen_futures::JsFuture;
    use web_sys::*;

    let ctx = AudioContext::new().map_err(map_js_err)?;
    let window = window().unwrap();
    let node = match AudioWorkletNode::new(&ctx, "red-siren-processor") {
        Ok(node) => node,
        Err(_) => {
            let worklet = ctx.audio_worklet().map_err(map_js_err)?;
            let loading = worklet
                .add_module("/worklet/au_core.js")
                .map_err(map_js_err)?;
            log::info!("load module");
            JsFuture::from(loading).await.map_err(map_js_err)?;
            AudioWorkletNode::new(&ctx, "red-siren-processor").map_err(map_js_err)?
        }
    };

    let response = JsFuture::from(window.fetch_with_str("/worklet/wasm/au_core_bg.wasm"))
        .await
        .map_err(map_js_err)?;
    let response = Response::from(response);
    let promise = response.array_buffer().map_err(map_js_err)?;
    let wasm_bytes = JsFuture::from(promise).await.map_err(map_js_err)?;

    let port = node.port().map_err(map_js_err)?;
    let mut send_bytes = |resolve: Function, _: Function| {
        let listener = Closure::wrap(Box::new(move |ev: JsValue| {
            let ev = MessageEvent::from(ev);
            let data = ev.data();
            let ev_type = Reflect::get(&data, &"type".into())
                .expect("ev type")
                .as_string()
                .expect("ev type");
            if ev_type.as_str() == "wasm_ready" {
                resolve.call0(&JsValue::NULL);
                log::info!("wasm loaded");
            }
        }) as Box<dyn FnMut(JsValue)>);

        port.set_onmessage(Some(listener.as_ref().unchecked_ref()));
        let message = Object::new();
        Reflect::set(&message, &"type".into(), &"wasm".into()).unwrap();
        Reflect::set(&message, &"value".into(), &wasm_bytes).unwrap();
        port.post_message(&message).unwrap();
        std::mem::forget(listener);
    };

    let ready_promise = Promise::new(&mut send_bytes);
    JsFuture::from(ready_promise).await.map_err(map_js_err)?;

    log::info!("setup audio node and ctx");

    let port = node.port().map_err(map_js_err)?;

    let on_message = Closure::wrap(Box::new(move |ev: JsValue| {
        let ev = MessageEvent::from(ev);
        let data = ev.data();
        let ev_type = Reflect::get(&data, &"type".into())
            .expect("ev type")
            .as_string()
            .expect("ev type");
        let value = Reflect::get(&data, &"value".into())
            .ok()
            .map(|v| Uint8Array::from(v));

        log::trace!("unit received ev: {}", ev_type.as_str());

        match (ev_type.as_str(), value) {
            ("snoops_data", Some(arr)) => {
                let snoops_data =
                    bincode::deserialize::<SnoopsData>(&arr.to_vec()).expect("deserialize");

                app_au_buffer.push_snoops_data(snoops_data);
            }
            ("fft_data", Some(arr)) => {
                let fft_data = bincode::deserialize::<FFTData>(&arr.to_vec()).expect("deserialize");
                app_au_buffer.push_fft_data(fft_data);
            }
            _ => {
                unimplemented!("event type: {ev_type}")
            }
        }
    }) as Box<dyn FnMut(JsValue)>);

    port.set_onmessage(Some(on_message.as_ref().unchecked_ref()));

    let dst = ctx.destination();
    node.connect_with_audio_node(&dst).map_err(map_js_err)?;

    let navigator = window.navigator();
    let md = navigator.media_devices().map_err(map_js_err)?;
    let mut constraints = MediaStreamConstraints::new();
    constraints.audio(&true.into());

    let query_device = md
        .get_user_media_with_constraints(&constraints)
        .map_err(map_js_err)?;

    let stream = JsFuture::from(query_device).await.map_err(map_js_err)?;
    let stream = MediaStream::from(stream);
    let src_options = MediaStreamAudioSourceOptions::new(&stream);
    let src = MediaStreamAudioSourceNode::new(&ctx, &src_options).map_err(map_js_err)?;

    src.connect_with_audio_node(&node).map_err(map_js_err)?;

    Ok((node, ctx, on_message))
}
