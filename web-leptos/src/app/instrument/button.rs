use app_core::geometry::Rect;
use leptos::{ev::PointerEvent, *};

#[component]
pub fn ButtonComponent(
    #[prop(into)] layout_rect: Signal<Rect>,
    #[prop(optional)] movement_xy: Option<Callback<(i32, i32)>>,
    #[prop(optional)] movement_x: Option<Callback<i32>>,
    #[prop(optional)] movement_y: Option<Callback<i32>>,
    #[prop(optional)] activation: Option<Callback<bool>>,
) -> impl IntoView {
    let r = move || layout_rect().width() / 2.0;
    let cx = move || layout_rect().center().x;
    let cy = move || layout_rect().center().y;

    let activate = Callback::new(move |e: PointerEvent| {
        log::debug!("down {e:?}");
        e.prevent_default();
        if let Some(cb) = activation {
            cb(true);
        }
    });

    let deactivate = Callback::new(move |e: PointerEvent| {
        log::debug!("up {e:?}");
        e.prevent_default();
        if let Some(cb) = activation {
            cb(false);
        }
    });

    let active_move = Callback::new(move |e: PointerEvent| {
        log::debug!("move {e:?}");
        e.prevent_default();

        let mx = e.movement_x();
        let my = e.movement_y();

        match (mx.abs() > my.abs(), movement_xy, movement_x, movement_y) {
            (_, Some(mxy_cb), None, None) => mxy_cb((e.client_x(), e.client_y())),
            (_, Some(_), Some(_), _) | (_, Some(_), _, Some(_)) => panic!("conflicting callbacks"),
            (_, _, Some(mx_cb), None) | (true, _, Some(mx_cb), _) => mx_cb(e.client_x()),
            (_, _, None, Some(my_cb)) | (false, _, _, Some(my_cb)) => my_cb(e.client_y()),
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
            on:pointerdown=activate
            on:pointermove=active_move
            on:pointerup=deactivate
            on:pointerleave=deactivate
        />
    }
}
