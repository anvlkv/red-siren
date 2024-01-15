use leptos::*;

use app_core::geometry::Line;
use mint::Point2;

#[component]
pub fn StringComponent(
    #[prop(into)] layout_line: Signal<Line>,
    #[prop(into, optional)] data: Signal<Vec<Point2<f64>>>,
) -> impl IntoView {
    let start = move || {
        let p0 = layout_line().p0();
        format!("M {},{}", p0.x, p0.y)
    };

    let end = move || {
        let p1 = layout_line().p1();
        format!("L {},{}", p1.x, p1.y)
    };
    
    let mid = move || {
        let mut ln = String::default();
        for pt in data() {
            ln.push_str(format!("L {}, {}", pt.x, pt.y).as_str())
        }
        ln
    };

    let d = move || format!("{} {} {}", start(), mid(), end());

    view! {
      <path d={d} />
    }
}
