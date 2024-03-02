use leptos::*;

use super::CoreContext;

#[component]
pub fn Objects() -> impl IntoView {
    let CoreContext(vm, _) = use_context::<CoreContext>().unwrap();

    let objects = move || vm().visual.objects;

    view! {
      <For each=objects key={|o| o.0.id} let:child>
        <Object obj=child.0 paint=child.1/>
      </For>
    }
}

#[component]
pub fn Object(obj: app_core::Object, paint: app_core::Paint) -> impl IntoView {
    let obj_view = match obj.shape {
        app_core::Shapes::Path(points) => {
            let d: String = points
                .iter()
                .map(|p| format!("L {}, {}", p.x, p.y))
                .collect();
            let p0 = points.first().cloned().unwrap_or_default();
            let d = format!("M {}, {} {}", p0.x, p0.y, d);

            view! {
                <path d={d}/>
            }
            .into_any()
        }
        app_core::Shapes::Circle(rect_box) => {
            let c = rect_box.center();
            let r = rect_box.width() / 2.0;
            view! {
                <circle cx={c.x}
                        cy={c.y}
                        r={r}/>
            }
            .into_any()
        }
        app_core::Shapes::RoundedRect(rect_box, rounded) => view! {
            <rect width={rect_box.width()}
                  height={rect_box.height()}
                  x={rect_box.min.x}
                  y={rect_box.min.y}
                  rx={rounded.width}
                  ry={rounded.height}/>
        }
        .into_any(),
    };

    let fill = paint
        .fill
        .map(|c| format!("rgba({},{},{},{})", c.r(), c.g(), c.b(), c.a()));
    let stroke = paint.stroke.as_ref().map(|s| {
        format!(
            "rgba({},{},{},{})",
            s.color.r(),
            s.color.g(),
            s.color.b(),
            s.color.a()
        )
    });
    let stroke_width = paint.stroke.map(|s| s.width);

    view! {
      <g fill={fill}
         stroke={stroke}
         stroke-width={stroke_width} >
        {obj_view}
      </g>
    }
}
