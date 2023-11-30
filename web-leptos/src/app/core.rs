use std::rc::Rc;

use leptos::{spawn_local, use_context, SignalUpdate, WriteSignal};
use leptos_router::NavigateOptions;
use shared::{
    navigate::NavigateOperation, Effect, Event, RedSiren, RedSirenCapabilities, ViewModel,
};


pub type Core = Rc<shared::Core<Effect, RedSiren>>;

pub fn new() -> Core {
    Rc::new(shared::Core::new::<RedSirenCapabilities>())
}

pub fn update(core: &Core, event: Event, render: WriteSignal<ViewModel>) {
    for effect in core.process_event(event) {
        process_effect(core, effect, render);
    }
}

pub fn process_effect(core: &Core, effect: Effect, render: WriteSignal<ViewModel>) {
    let navigate = leptos_router::use_navigate();

    match effect {
        Effect::Render(_) => {
            render.update(|view| *view = core.view());
        }
        Effect::KeyValue(mut req) => {
            // let response = match &req.operation {
            //     shared::key_value::KeyValueOperation::Read(key) => {
            //         shared::key_value::KeyValueOutput::Read(kv_ctx.borrow_mut().remove(key))
            //     }
            //     shared::key_value::KeyValueOperation::Write(key, data) => {
            //         let _ = kv_ctx.borrow_mut().insert(key.clone(), data.clone());
            //         shared::key_value::KeyValueOutput::Write(true)
            //     }
            // };

            // for effect in core.resolve(&mut req, response) {
            //     process_effect(&core, effect, render);
            // }
        }
        Effect::Navigate(nav) => match nav.operation {
            NavigateOperation::To(activity) => match activity {
                shared::Activity::Intro => navigate("/", NavigateOptions::default()),
                shared::Activity::Tune => navigate("/tune", NavigateOptions::default()),
                shared::Activity::Play => navigate("/play", NavigateOptions::default()),
                shared::Activity::Listen => navigate("/listen", NavigateOptions::default()),
            },
        },
    };
}
