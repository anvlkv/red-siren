use std::{rc::Rc, cell::RefCell, collections::HashSet};

use leptos::*;
use leptos_meta::Title;

pub use super::instrument::ButtonComponent;
use shared::{geometry::Line, tuner};
// pub use string::StringComponent;
// pub use track::TrackComponent;

// use super::menu::MenuComponent;

#[component]
pub fn TunerLine(#[prop(into)] layout_line: Signal<Line>, #[prop(into)] fft: Signal<Vec<(f32, f32)>>) -> impl IntoView {
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
      let x_base = layout_line().p0().x;
      let y_base = layout_line().p0().y;
      let step = layout_line().width() / fft().len() as f64;
      for (i, (_, val)) in fft().iter().enumerate() {
        let x = x_base + step * i as f64;
        let y = y_base + (*val as f64);
        ln.push_str(format!("L {x}, {y}").as_str())
      }
      ln
    };

    let d = move || format!("{} {} {}", start(), mid(), end());

    view! {
      <path d={d} />
    }
}

#[component]
pub fn TunerComponent(
    view_box: Signal<String>,
    vm: Signal<tuner::TunerVM>,
    ev: SignalSetter<tuner::TunerEV>,
) -> impl IntoView {
    let layout_line = Signal::derive(move || vm().line);
    // let outbound_layout_line = Signal::derive(move || vm().layout.outbound);

    // let playing = Signal::derive(move || vm().playing);

    // let menu_position = Signal::derive(move || vm().layout.menu_position);
    let range = move || vm().range;

    let pairs = move || {
        vm().pairs.into_iter().map(move |pair| {
            let f_n = pair.f_n;
            (
                f_n,
                Signal::derive(move || pair.rect),
            )
        })
    };

    let pressed_buttons = Rc::new(RefCell::new(HashSet::<usize>::new()));

    let fft = Signal::derive(move || vm().fft);
    log::debug!("render tuner");

    view! {
      <div class="h-full w-full bg-red dark:bg-black instrument">
        <Title text="Red Siren - Tune"/>
        <svg fill="none" class="stroke-black dark:stroke-red" viewBox={view_box} xmlns="http://www.w3.org/2000/svg">
          <TunerLine layout_line=layout_line fft=fft/>
        </svg>
        <svg class="fill-black dark:fill-red" viewBox={view_box} xmlns="http://www.w3.org/2000/svg">
          {move || pairs().map(|child| {
            let act_pressed = pressed_buttons.clone();
            let mv_y_pressed = pressed_buttons.clone();
            let f_n = child.0;
            view!{
              <ButtonComponent layout_rect={child.1} f_n={f_n} 
                activation={
                  Callback::new(move |val| {
                    if val {
                      _ = act_pressed.borrow_mut().insert(f_n)
                    }
                    else {
                      _ = act_pressed.borrow_mut().remove(&f_n)
                    }
                  })
                } 
                movement_y={
                  Callback::new(move |val| {
                    if mv_y_pressed.borrow().contains(&f_n) {
                      let tuner_value = (range() / 2.0 - val as f64) * (2.0 / range());
                      ev.set(tuner::TunerEV::SetTuningValue(child.0, tuner_value as f32))
                    }
                  })
                }
              />
            }
        }).collect_view()}            
        </svg>
        // <MenuComponent position={menu_position} playing=playing />
      </div>
    }
}
