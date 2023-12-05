use leptos::*;
use shared::geometry::Rect;

#[component]
pub fn ButtonComponent(
    #[prop(into)] layout_rect: Signal<Rect>,
    #[prop(optional)] f_n: Option<usize>,
) -> impl IntoView {
    let r = move || layout_rect().width() / 2.0;
    let cx = move || layout_rect().center().x;
    let cy = move || layout_rect().center().y;

    let label = f_n.map(|f| {
        let l = format!("f{f}");
        view! {<text fill="black" x=cx y=cy>{l}</text>}
    });

    view! {
      <circle r=r cx=cx cy=cy />
      {label}
    }
}
