mod button;
mod string;
mod track;
mod menu;
mod playback;

use leptos::*;
use shared::{instrument, key_value::KeyValueOutput};

pub use button::ButtonComponent;
pub use menu::MenuComponent;
pub use string::StringComponent;
pub use track::TrackComponent;

use crate::{app::instrument::playback::PlayBackState, kv::KVContext};

#[component]
pub fn InstrumentComponent(
    vm: Signal<instrument::InstrumentVM>,
    ev: SignalSetter<instrument::InstrumentEV>,
) -> impl IntoView {
    let view_box = move || {
        let vb = vm().view_box;
        format!(
            "{} {} {} {}",
            vb.top_left().x,
            vb.top_left().y,
            vb.bottom_right().x,
            vb.bottom_right().y
        )
    };

    let inbound_layout_line = Signal::derive(move || vm().layout.inbound);
    let outbound_layout_line = Signal::derive(move || vm().layout.outbound);

    let playing = Signal::derive(move || vm().playing);
    let config = Signal::derive(move || vm().config);
    let playback_ev = SignalSetter::map(move |e| ev.set(instrument::InstrumentEV::Playback(e)));
    let playback_state = playback::create_playback(playing.clone(), config, playback_ev);

    // create_effect(move |prev| {
    //     let state = playback_state.get();

    //     match (state, prev) {
    //         (PlayBackState::Playing, Some(PlayBackState::Preparing)) => ev(instrument::InstrumentEV::Input(
    //             KeyValueOutput::Read(kv_ref.borrow_mut().remove(instrument::INPUT_STREAM_KV)),
    //         )),
    //         _ => {}
    //     }

    //     state
    // });

    let toggle_playing = Callback::<()>::new(move |_| {
        ev(instrument::InstrumentEV::Playback(
            instrument::PlaybackEV::Play(!playing()),
        ))
    });
    let menu_position = Signal::derive(move || vm().layout.menu_position);

    view! {
      <div class="h-full w-full bg-red dark:bg-black instrument">
        <svg fill="none" class="stroke-black dark:stroke-red" viewBox={view_box} xmlns="http://www.w3.org/2000/svg">
          <StringComponent layout_line={inbound_layout_line} />
          <StringComponent layout_line={outbound_layout_line} />
        </svg>
        <svg class="fill-red dark:fill-black stroke-black dark:stroke-red" viewBox={view_box} xmlns="http://www.w3.org/2000/svg">
          {move || vm().layout.tracks.into_iter().zip(vm().nodes).map(|(rect, node)|
            view!{
              <TrackComponent layout_rect={Signal::derive(move || rect)}/>
            }
          ).collect_view()}
        </svg>
        <svg class="fill-black dark:fill-red" viewBox={view_box} xmlns="http://www.w3.org/2000/svg">
          {move || vm().layout.buttons.into_iter().zip(vm().nodes).map(|(rect, node)|
            view!{
              <ButtonComponent layout_rect={Signal::derive(move || rect)} f_n={node.f_n}/>
            }
          ).collect_view()}
        </svg>
        <MenuComponent position={menu_position} playing=playing toggle_playing=toggle_playing />
      </div>
    }
}
