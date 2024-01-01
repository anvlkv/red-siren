use leptos::{ev::PointerEvent, *};
use shared::geometry::Rect;

#[component]
pub fn ButtonComponent(
    #[prop(into)] layout_rect: Signal<Rect>,
    #[prop(optional)] f_n: Option<usize>,
    #[prop(optional)] movement_x: Option<Callback<i32>>,
    #[prop(optional)] movement_y: Option<Callback<i32>>,
    #[prop(optional)] activation: Option<Callback<bool>>,
) -> impl IntoView {
    let r = move || layout_rect().width() / 2.0;
    let cx = move || layout_rect().center().x;
    let cy = move || layout_rect().center().y;

    let label = f_n.map(|f| {
        let l = format!("f{f}");
        view! {<text fill="black" x=cx y=cy>{l}</text>}
    });

    let pointerdown = Callback::new(move |e: PointerEvent| {
        log::debug!("down {e:?}");
        e.prevent_default();
        if let Some(cb) = activation {
            cb(true);
        }
    });

    let pointerup = Callback::new(move |e: PointerEvent| {
        log::debug!("up {e:?}");
        e.prevent_default();
        if let Some(cb) = activation {
            cb(false);
        }
    });

    let pointermove = Callback::new(move |e: PointerEvent| {
        log::debug!("move {e:?}");
        e.prevent_default();

        let mx = e.movement_x();
        let my = e.movement_y();

        match (mx.abs() > my.abs(), movement_x, movement_y) {
            (_, Some(mx_cb), None) | (true, Some(mx_cb), _) => mx_cb(e.client_x()),
            (_, None, Some(my_cb)) | (false, _, Some(my_cb)) => my_cb(e.client_y()),
            _ => {
                log::warn!("no callback")
            }
        }
    });

    view! {
      <circle class="instrument-button"
        r=r
        cx=cx
        cy=cy
        on:pointerdown=pointerdown
        on:pointermove=pointermove
        on:pointerup=pointerup
      />
      {label}
    }
}
