use std::rc::Rc;

use leptos::*;

use shared::{
    navigate::NavigateOperation, Effect, Event, RedSiren, RedSirenCapabilities, ViewModel,
};

use super::playback;

pub type Core = Rc<shared::Core<Effect, RedSiren>>;

pub fn new() -> Core {
    Rc::new(shared::Core::new::<RedSirenCapabilities>())
}

pub fn update(
    core: &Core,
    event: Event,
    render: WriteSignal<ViewModel>,
    playback: playback::Playback,
) {
    for effect in core.process_event(event) {
        process_effect(core, effect, render, playback.clone());
    }
}

#[allow(unused_variables)]
pub fn process_effect(
    core: &Core,
    effect: Effect,
    render: WriteSignal<ViewModel>,
    playback: playback::Playback,
) {
    match effect {
        Effect::Render(_) => {
            render.update(|view| *view = core.view());
        }
        Effect::KeyValue(_req) => {
            // let response = match &req.operation {
            //     shared::key_value::KeyValueOperation::Read(key) => {
            //         shared::key_value::KeyValueOutput::Read(kv_ctx.borrow_mut().remove(key))
            //     }
            //     shared::key_value::KeyValueOperation::Write(key, data) => {
            //         _ = kv_ctx.borrow_mut().insert(key.clone(), data.clone());
            //         shared::key_value::KeyValueOutput::Write(true)
            //     }
            // };
        }
        Effect::Navigate(nav) => match nav.operation {
            NavigateOperation::To(activity) => {
                update(core, Event::Activate(activity), render, playback)
            }
        },
        #[allow(unused_mut, unused_variables)]
        Effect::Play(mut req) => {
            #[cfg(feature = "browser")]
            {
                let core = core.clone();
                let playback = playback.clone();
                spawn_local(async move {
                    let response = playback.request(req.operation.clone()).await;

                    for effect in core.resolve(&mut req, response) {
                        process_effect(&core, effect, render, playback.clone());
                    }
                })
            }
        }
    };
}
