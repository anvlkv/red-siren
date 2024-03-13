use std::rc::Rc;

#[allow(unused)]
use app_core::{
    Activity, AnimateOperation, AnimateOperationOutput, Effect, Event, Operation, RedSiren,
    RedSirenCapabilities, Request, UnitResolve, ViewModel,
};
use futures::{
    channel::mpsc::{channel, Sender},
    StreamExt,
};
use leptos::*;

pub type Core = Rc<app_core::Core<Effect, RedSiren>>;

pub fn new() -> Core {
    Rc::new(app_core::Core::new::<RedSirenCapabilities>())
}

pub fn update(
    core: &Core,
    event: Event,
    render: WriteSignal<ViewModel>,
    navigate: Callback<&str>,
    animate: Callback<Option<Sender<f64>>>,
) {
    for effect in core.process_event(event) {
        process_effect(core, effect, render, navigate, animate);
    }
}

fn resolve<Op>(
    req: &mut Request<Op>,
    value: Op::Output,
    core: &Core,
    render: WriteSignal<ViewModel>,
    navigate: Callback<&str>,
    animate: Callback<Option<Sender<f64>>>,
) where
    Op: Operation,
{
    for effect in core.resolve(req, value) {
        process_effect(&core, effect, render, navigate, animate);
    }
}

#[allow(unused_variables)]
pub fn process_effect(
    core: &Core,
    effect: Effect,
    render: WriteSignal<ViewModel>,
    navigate: Callback<&str>,
    animate: Callback<Option<Sender<f64>>>,
) {
    match effect {
        Effect::Render(_) => {
            render.update(|view| *view = core.view());
        }
        #[allow(unused_mut)]
        Effect::Play(mut req) => match req.operation {
            app_core::PlayOperation::Permissions => {
                #[cfg(feature = "browser")]
                {
                    use js_sys::{Object, Reflect};
                    use wasm_bindgen_futures::JsFuture;
                    use web_sys::{
                        window, MediaStreamConstraints, PermissionState, PermissionStatus,
                    };

                    let win = window().unwrap();
                    let navigator = win.navigator();
                    if let Some(promise) = navigator
                        .permissions()
                        .ok()
                        .map(|perms| {
                            let q = Object::new();
                            Reflect::set(&q, &"name".into(), &"microphone".into()).unwrap();
                            perms.query(&q).ok()
                        })
                        .flatten()
                    {
                        let core = core.clone();
                        spawn_local(async move {
                            let result = JsFuture::from(promise).await.unwrap();
                            let status = PermissionStatus::from(result);
                            let grant = match status.state() {
                                PermissionState::Granted => true,
                                PermissionState::Denied => false,
                                PermissionState::Prompt => {
                                    if let Some(promise) = navigator
                                        .media_devices()
                                        .ok()
                                        .map(|md| {
                                            let mut constraints = MediaStreamConstraints::new();
                                            constraints.audio(&true.into());
                                            md.get_user_media_with_constraints(&constraints).ok()
                                        })
                                        .flatten()
                                    {
                                        match JsFuture::from(promise).await {
                                            Ok(_) => true,
                                            Err(_) => false,
                                        }
                                    } else {
                                        false
                                    }
                                }
                                _ => false,
                            };

                            resolve(
                                &mut req,
                                UnitResolve::RecordingPermission(grant),
                                &core,
                                render,
                                navigate,
                                animate,
                            );
                        })
                    } else {
                        resolve(
                            &mut req,
                            UnitResolve::RecordingPermission(false),
                            &core,
                            render,
                            navigate,
                            animate,
                        );
                    }
                }
            }
            app_core::PlayOperation::RunUnit => {
                log::info!("run unit");
            }
        },

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
                log::info!("web: req start animation");

                spawn_local(async move {
                    while let Some(ts) = rx.next().await {
                        resolve(
                            &mut req,
                            AnimateOperationOutput::Timestamp(ts),
                            &core,
                            render,
                            navigate,
                            animate,
                        );
                    }

                    resolve(
                        &mut req,
                        AnimateOperationOutput::Done,
                        &core,
                        render,
                        navigate,
                        animate,
                    );
                });

                animate(Some(sx));
            }
            AnimateOperation::Stop => {
                animate(None);

                resolve(
                    &mut req,
                    AnimateOperationOutput::Done,
                    &core,
                    render,
                    navigate,
                    animate,
                );
            }
        },
    };
}
