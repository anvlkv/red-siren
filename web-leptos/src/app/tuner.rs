use leptos::*;
use leptos_meta::Title;
use mint::Point2;

pub use super::instrument::ButtonComponent;
use app_core::{geometry::Line, tuner, Event};

use super::red_card::RedCardComponent;

#[component]
pub fn TunerLine(
    #[prop(into)] layout_line: Signal<Line>,
    #[prop(into)] fft: Signal<Vec<Point2<f64>>>,
) -> impl IntoView {
    let start = move || {
        let p0 = layout_line().p1();
        format!("M {},{}", p0.x, p0.y)
    };

    let end = move || {
        let p1 = layout_line().p0();
        format!("L {},{}", p1.x, p1.y)
    };

    let mid = move || {
        let mut ln = String::default();
        for pt in fft() {
            ln.push_str(format!("L {}, {}", pt.x, pt.y).as_str())
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
    let ev_ctx = use_context::<WriteSignal<Event>>().expect("root ev context");
    let pairs = move || {
        vm().pairs.into_iter().map(move |pair| {
            let f_n = pair.f_n;
            (f_n, Signal::derive(move || pair.rect))
        })
    };

    let fft = Signal::derive(move || vm().fft);
    let fft_max = Signal::derive(move || vm().fft_max);
    let menu_position = Signal::derive(move || vm().menu_position);
    let btn_class = "w-full rounded-2xl bg-red dark:bg-black text-black dark:text-red text-xl hover:text-gray dark:hover:text-cinnabar";

    view! {
      <div class="h-full w-full bg-red dark:bg-black instrument">
        <Title text="Red Siren - Tune"/>
        <svg fill="none" class="stroke-black dark:stroke-red" viewBox={view_box} xmlns="http://www.w3.org/2000/svg">
          <TunerLine layout_line=layout_line fft=fft/>
          <TunerLine layout_line=layout_line fft=fft_max/>
        </svg>
        <svg class="fill-black dark:fill-red" viewBox={view_box} xmlns="http://www.w3.org/2000/svg">
          {move || pairs().map(|child| {
            let f_n = child.0;
            view!{
              <ButtonComponent layout_rect={child.1}
                activation={
                  Callback::new(move |val: bool| {
                    ev.set(tuner::TunerEV::ButtonPress(f_n, val))
                  })
                }
                movement_xy ={
                  Callback::new(move |(x, y): (i32, i32)| {
                    ev.set(tuner::TunerEV::SetFreqAmpXYPos(f_n, x as f64, y as f64))
                  })
                }
              />
            }
        }).collect_view()}
        </svg>
        <RedCardComponent position={menu_position} style={move || "padding: .12rem".to_string()}>
          <button class=btn_class 
            disabled={move || vm().needs_tuning} 
            on:click=move|_| ev_ctx.set(Event::Menu(app_core::Activity::Play))>
            {"Done"}
          </button>
        </RedCardComponent>
      </div>
    }
}
