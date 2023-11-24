use std::{cell::RefCell, rc::Rc};

use leptos::*;
use leptos_use::use_window;
use shared::{
    geometry::{Line, Rect},
    instrument,
};

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
        view! {<text x=cx y=cy>{l}</text>}
    });

    view! {
      <circle r=r cx=cx cy=cy />
      {label}
    }
}

#[component]
pub fn TrackComponent(#[prop(into)] layout_rect: Signal<Rect>) -> impl IntoView {
    let r = move || layout_rect().width().min(layout_rect().height()) / 2.0;
    let p0 = move || layout_rect().top_left();
    let width = move || layout_rect().width();
    let height = move || layout_rect().height();

    view! {
      <rect x={move||p0().x} y={move||p0().y} width=width height=height rx=r ry=r/>
    }
}

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

    let base_freq = move || {
        vm.get()
            .nodes
            .into_iter()
            .map(|n| n.base_freq as f32)
            .collect::<Vec<_>>()
    };

    let playing = move || vm().playing;

    #[cfg(feature = "browser")]
    {
        let running_ctx: Rc<RefCell<Option<web_sys::AudioContext>>> = Rc::new(RefCell::new(None));
        let gains: Rc<RefCell<Vec<web_sys::GainNode>>> = Rc::new(RefCell::new(vec![]));

        create_effect(move |_| {
            if let Some(prev) = running_ctx.borrow_mut().as_mut() {
                prev.close().unwrap();
            }

            if (playing()) {
                let mut next_osc = vec![];
                let audio_ctx = match web_sys::AudioContext::new() {
                    Ok(ctx) => ctx,
                    Err(e) => {
                        log::error!("failed to create audio ctx {e:?}");
                        panic!();
                    }
                };

                let ts = audio_ctx.current_time();
                let mut next_gains = vec![];

                for freq in base_freq() {
                    let osc = match audio_ctx.create_oscillator() {
                        Ok(o) => o,
                        Err(e) => {
                            log::error!("failed to create oscillator {e:?}");
                            panic!();
                        }
                    };

                    let gain = match audio_ctx.create_gain() {
                        Ok(g) => g,
                        Err(e) => {
                            log::error!("failed to create gain {e:?}");
                            panic!();
                        }
                    };

                    gain.gain().set_value(1.0);

                    osc.set_type(web_sys::OscillatorType::Sine);
                    osc.frequency().set_value(freq);

                    osc.connect_with_audio_node(&gain).unwrap();

                    gain.connect_with_audio_node(&audio_ctx.destination())
                        .unwrap();

                    next_gains.push(gain);

                    match osc.start_with_when(ts) {
                        Ok(_) => {
                            next_osc.push(osc);
                            log::info!("added {freq}");
                        }
                        Err(e) => {
                            log::error!("failed to start oscillator {e:?}");
                            panic!();
                        }
                    };
                }
                
                let _ = audio_ctx.resume().unwrap();

                let _ = running_ctx.borrow_mut().insert(audio_ctx);
                let _ = gains.replace(next_gains);

            }
        });
    }

    let menu_style = move || {
        let vm = vm();
        let rect = vm.layout.menu_position.rect();
        format!(
            "width: {}px; height: {}px; top: {}px; left: {}px",
            rect.width(),
            rect.height(),
            rect.top_left().y,
            rect.top_left().x
        )
    };

    let menu_class = move || {
        let corner = match vm().layout.menu_position {
            instrument::layout::MenuPosition::TopLeft(_) => "top-left",
            instrument::layout::MenuPosition::TopRight(_) => "top-right",
            instrument::layout::MenuPosition::BottomLeft(_) => "bottom-left",
        };

        format!("absolute bg-red menu menu-{corner}")
    };

    let play = move |_| ev(instrument::InstrumentEV::Playback(!playing()));

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
        <div class={menu_class} style={menu_style}>
          <button on:click={play}>{"Play"}</button>
        </div>
      </div>
    }
}
