use leptos::*;
use leptos_meta::Title;

use app_core::instrument;
pub use button::ButtonComponent;
use leptos_use::{use_raf_fn_with_options, utils::Pausable, UseRafFnOptions};
pub use string::StringComponent;
pub use track::TrackComponent;

use super::menu::MenuComponent;

mod button;
mod string;
mod track;

#[component]
#[allow(unused_variables)]
pub fn InstrumentComponent(
    view_box: Signal<String>,
    vm: Signal<instrument::InstrumentVM>,
    ev: SignalSetter<instrument::InstrumentEV>,
) -> impl IntoView {
    let inbound_layout_line = Signal::derive(move || vm().layout.inbound);
    let outbound_layout_line = Signal::derive(move || vm().layout.outbound);
    let outbound_data = Signal::derive(move || vm().data_out);

    let playing = Signal::derive(move || vm().playing);

    let (ts, set_ts) = create_signal(0.0);
    let Pausable {
        resume,
        pause,
        is_active,
    } = use_raf_fn_with_options(
        move |args| {
          set_ts(args.timestamp);
        },
        UseRafFnOptions::default().immediate(false),
    );

    create_effect(move |prev| {
      let now = ts();
      if prev.map_or(true, |p| now - p >= 42.0 ) {
        ev(instrument::InstrumentEV::RequestSnoops);
        now
      }
      else {
        log::warn!("skip ts: {prev:?}");
        prev.unwrap_or(0.0)
      }
    });



    create_effect(move |_| {
        if playing() {
            resume()
        } else if is_active() {
            pause()
        }
    });

    let menu_position = Signal::derive(move || vm().layout.menu_position);

    let buttons = move || {
        vm().layout
            .buttons
            .into_iter()
            .zip(vm().nodes)
            .map(|(rect, node)| {
                view! {
                  <ButtonComponent
                    layout_rect={Signal::derive(move || rect)}
                    activation={Signal::derive(move || node.triggered)}
                  >
                    <p>
                      {move || format!("f{}",node.f_n)}
                    </p>
                  </ButtonComponent>
                }
            })
            .collect_view()
    };

    view! {
      <div class="h-full w-full bg-red dark:bg-black instrument">
        <Title text="Red Siren - Play"/>
        <svg fill="none" class="stroke-black dark:stroke-red" viewBox={view_box} xmlns="http://www.w3.org/2000/svg">
          <StringComponent layout_line={inbound_layout_line} />
          <StringComponent layout_line={outbound_layout_line} data={outbound_data}/>
        </svg>
        <svg class="fill-red dark:fill-black stroke-black dark:stroke-red" viewBox={view_box} xmlns="http://www.w3.org/2000/svg">
          {move || vm().layout.tracks.into_iter().zip(vm().nodes).map(|(rect, _node)|
            view!{
              <TrackComponent layout_rect={Signal::derive(move || rect)}/>
            }
          ).collect_view()}
        </svg>
        <div class="w-full h-full relative">
          {buttons}
        </div>
        <MenuComponent position={menu_position} playing=playing />
      </div>
    }
}
