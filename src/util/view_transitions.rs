use leptos::ev::Custom;
use leptos_use::{use_document, use_event_listener};
use web_sys::Event;

/// Provide a document-level toggler for a `vt-active` class on `<body>`
/// during View Transitions to prevent blur artifacts.
///
/// How it works:
/// - On "viewtransitionstart": add `vt-active` to `<body>`
/// - On "viewtransitionend"/"viewtransitioncancel": remove `vt-active`
///
/// CSS can then temporarily disable backdrop-filter on live cards and/or
/// adjust other properties while the transition runs, without removing
/// the card's own styles permanently.
///
/// Usage:
/// - Call this early in app startup (e.g., in `main()` before mount).
pub fn provide_view_transition_class_toggler() {
    let document = use_document();
    if let Some(body) = document.body() {
        // Start: add vt-active
        {
            let body = body.clone();
            let ev_start = Custom::<Event>::new("viewtransitionstart");
            let _dispose = use_event_listener::<Custom<Event>, _, _, _>(
                document.clone(),
                ev_start,
                move |_evt: Event| {
                    let _ = body.class_list().add_1("vt-active");
                },
            );
        }

        // End: remove vt-active
        {
            let body = body.clone();
            let ev_end = Custom::<Event>::new("viewtransitionend");
            let _dispose = use_event_listener::<Custom<Event>, _, _, _>(
                document.clone(),
                ev_end,
                move |_evt: Event| {
                    let _ = body.class_list().remove_1("vt-active");
                },
            );
        }

        // Cancel: remove vt-active
        {
            let body = body.clone();
            let ev_cancel = Custom::<Event>::new("viewtransitioncancel");
            let _dispose = use_event_listener::<Custom<Event>, _, _, _>(
                document,
                ev_cancel,
                move |_evt: Event| {
                    let _ = body.class_list().remove_1("vt-active");
                },
            );
        }
    }
}
