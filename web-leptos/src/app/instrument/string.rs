use leptos::*;

use shared::geometry::Line;

#[component]
pub fn StringComponent(#[prop(into)] layout_line: Signal<Line>) -> impl IntoView {
    let start = move || {
        let p0 = layout_line().p0();
        format!("M {},{}", p0.x, p0.y)
    };

    let end = move || {
        let p1 = layout_line().p1();
        format!("L {},{}", p1.x, p1.y)
    };
    let d = move || format!("{} {}", start(), end());

    view! {
      <path d={d} />
    }
}
