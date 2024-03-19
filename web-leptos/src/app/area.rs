use leptos::*;
use super::CoreContext;

#[component]
pub fn Area(children: Children) -> impl IntoView {
  let CoreContext(vm, ev) = use_context::<CoreContext>().unwrap();
  let view_box = move || format!("0 0 {} {}", vm().visual.width, vm().visual.height);
  view!{
    <svg class="area" view_box=view_box xmlns="http://www.w3.org/2000/svg">
      {children()}
    </svg>
  }
}