use leptos::{ev::PointerEvent, *};
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
    let pairs = Signal::derive(move || {
        vm().pairs
            .into_iter()
            .map(move |pair| {
                let f_n = pair.f_n;
                (f_n, Signal::derive(move || pair))
            })
            .collect::<Vec<_>>()
    });

    let fft = Signal::derive(move || vm().fft);
    let fft_max = Signal::derive(move || vm().fft_max);
    let menu_position = Signal::derive(move || vm().menu_position);
    let btn_class = "w-full rounded-2xl bg-red dark:bg-black text-black dark:text-red text-xl hover:text-gray dark:hover:text-cinnabar";

    let activate = Callback::new(move |e: PointerEvent| {
        log::debug!("down {e:?}");
        e.prevent_default();
        let c = (e.client_x() as f64, e.client_y() as f64);
        ev.set(tuner::TunerEV::ActivationXY(c, e.pointer_id()))
    });

    let deactivate = Callback::new(move |e: PointerEvent| {
        log::debug!("up {e:?}");
        e.prevent_default();
        ev.set(tuner::TunerEV::DeactivationXY(e.pointer_id()))
    });

    let active_move = Callback::new(move |e: PointerEvent| {
        log::debug!("move {e:?}");
        e.prevent_default();
        let c = (e.client_x() as f64, e.client_y() as f64);
        ev.set(tuner::TunerEV::MovementXY(c, e.pointer_id()))
    });

    view! {
      <div class="h-full w-full bg-red dark:bg-black instrument">
        <Title text="Red Siren - Tune"/>
        <svg fill="none" class="stroke-black dark:stroke-red" viewBox={view_box} xmlns="http://www.w3.org/2000/svg">
          <TunerLine layout_line=layout_line fft=fft/>
          <TunerLine layout_line=layout_line fft=fft_max/>
        </svg>
        <div class="w-full h-full relative"
          on:pointerdown=activate
          on:pointerup=deactivate
          on:pointermove=active_move>
          {
            move || pairs().into_iter().map(|(f_n, pair )| {
            view!{
              <ButtonComponent layout_rect={Signal::derive(move || pair().rect)} activation={Signal::derive(move || match pair().triggered {
                tuner::TriggerState::None => 0.5,
                tuner::TriggerState::Ghost => -0.5,
                tuner::TriggerState::Active => -1.0,
              })}>
                <p>
                  {move || format!("f{}", pair().f_n)}
                </p>
                <Show when=move || pair().value.is_some()>
                  <p>
                    {move || format!("{}hz, {:01.3}", pair().value.unwrap().0 as usize, pair().value.unwrap().1)}
                  </p>
                </Show>
              </ButtonComponent>
            }}).collect_view()
          }
        </div>
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
