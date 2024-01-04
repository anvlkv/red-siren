use leptos::*;

use app_core::geometry::Rect;

#[component]
pub fn TrackComponent(#[prop(into)] layout_rect: Signal<Rect>) -> impl IntoView {
    let r = move || layout_rect().width().min(layout_rect().height()) / 2.0;
    let p0 = move || layout_rect().top_left();
    let width = move || layout_rect().width();
    let height = move || layout_rect().height();

    view! {
      <rect x={move||p0().x} y={move||p0().y} width=width height=height rx=r ry=r/>
    }
}
