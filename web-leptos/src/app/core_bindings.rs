use futures::{
    channel::mpsc::{channel, Sender},
    StreamExt,
};
use std::rc::Rc;

use leptos::*;

use app_core::{AnimateOperation, AnimateOperationOutput, PlayOperationOutput};
use app_core::{
    Activity, Effect, Event, RedSiren, RedSirenCapabilities, ViewModel,
};


pub type Core = Rc<app_core::Core<Effect, RedSiren>>;

pub fn new() -> Core {
    Rc::new(app_core::Core::new::<RedSirenCapabilities>())
}

pub fn update(
    core: &Core,
    event: Event,
    render: WriteSignal<ViewModel>,
    navigate: Callback<&str>,
    animate_cb: Callback<Option<Sender<f64>>>,
) {
    for effect in core.process_event(event) {
        process_effect(core, effect, render,  navigate, animate_cb);
    }
}

#[allow(unused_variables)]
pub fn process_effect(
    core: &Core,
    effect: Effect,
    render: WriteSignal<ViewModel>,
    navigate: Callback<&str>,
    animate_cb: Callback<Option<Sender<f64>>>,
) {
    match effect {
        Effect::Render(_) => {
            render.update(|view| *view = core.view());
        }
        Effect::Play(mut req) => {
            // match req.operation {
            //     app_core::PlayOperation::Permissions => {

            //     },
            //     app_core::PlayOperation::InstallAU => todo!(),
            // }
            log::info!("play op: {:?}", req.operation);

            core.resolve(&mut req, PlayOperationOutput::Success);
        }
        
        // Effect::KeyValue(mut req) => {
        //     #[cfg(feature = "browser")]
        //     {
        //         use gloo_storage::{LocalStorage, Storage};

        //         let response = match &req.operation {
        //             app_core::key_value::KeyValueOperation::Read(key) => {
        //                 app_core::key_value::KeyValueOutput::Read(LocalStorage::get(key).ok())
        //             }
        //             app_core::key_value::KeyValueOperation::Write(key, data) => {
        //                 app_core::key_value::KeyValueOutput::Write(
        //                     LocalStorage::set(key, data).is_ok(),
        //                 )
        //             }
        //         };

        //         for effect in core.resolve(&mut req, response) {
        //             process_effect(
        //                 &core,
        //                 effect,
        //                 render,
        //                 navigate,
        //                 animate_cb,
        //             );
        //         }
        //     }
        // }
        // Effect::Navigate(nav) => match nav.operation {
        //     NavigateOperation::To(activity) => {
        //         let path = match activity {
        //             Activity::Intro => "/",
        //             Activity::Tune => "/tune",
        //             Activity::Play => "/play",
        //             Activity::Listen => "/listen",
        //             Activity::About => "/about",
        //         };

        //         navigate(path);

        //         update(
        //             core,
        //             Event::ReflectActivity(activity),
        //             render,
        //             navigate,
        //             animate_cb,
        //         );
        //     }
        // },
        Effect::Animate(mut req) => match req.operation {
            AnimateOperation::Start => {
                let (sx, mut rx) = channel::<f64>(1);
                let core = core.clone();
                spawn_local(async move {
                    while let Some(ts) = rx.next().await {
                        for effect in core.resolve(&mut req, AnimateOperationOutput::Timestamp(ts))
                        {
                            process_effect(
                                &core,
                                effect,
                                render,
                                navigate,
                                animate_cb,
                            );
                        }
                    }

                    for effect in core.resolve(&mut req, AnimateOperationOutput::Done) {
                        process_effect(
                            &core,
                            effect,
                            render,
                            navigate,
                            animate_cb,
                        );
                    }

                    log::debug!("receive ts ended");
                });

                animate_cb(Some(sx));
            }
            AnimateOperation::Stop => animate_cb(None),
        },
    };
}
