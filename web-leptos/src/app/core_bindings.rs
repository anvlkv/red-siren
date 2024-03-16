use std::rc::Rc;

#[allow(unused)]
use bridge::app_core::{
    Activity, AnimateOperation, AnimateOperationOutput, Event as AppEvent, Operation, ViewModel,
};
use bridge::{
    app_core::{PermissionsOperation, PermissionsOperationOutput},
    Coer, Effect, Event, RedSirenBridge, RedSirenBridgeCapabilities, ShellOp, ShellOutput,
};
use futures::{
    channel::mpsc::{channel, Sender},
    StreamExt,
};
use leptos::*;

pub type Core = Rc<Core<Effect, RedSirenBridge>>;

pub fn new() -> Core {
    Rc::new(Core::new::<RedSirenBridgeCapabilities>())
}

pub fn update(
    core: &Core,
    event: AppEvent,
    render: WriteSignal<ViewModel>,
    navigate: Callback<&str>,
    animate: Callback<Option<Sender<f64>>>,
) {
    for effect in core.process_event(Event::AppEvent(event)) {
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
        Effect::ShellCapability(req) => match &req.operation {
            ShellOp::Animate(op) => match op {
                AnimateOperation::Start => {
                    start_animation(&mut req, core, render, navigate, animate)
                }
                AnimateOperation::Stop => stop_animation(&mut req, core, render, navigate, animate),
            },
            ShellOp::Permissions(op) => match op {
                PermissionsOperation::AudioRecording => {
                    audio_recording_premissions(&mut req, core, render, navigate, animate)
                }
            },
        },
        Effect::Platform(mut req) => resolve(
            &mut req,
            PlatformResponse("web"),
            core,
            render,
            navigate,
            animate,
        ),
    };
}

fn stop_animation(
    req: &mut Request<ShellOp>,
    core: &Core,
    render: WriteSignal<ViewModel>,
    navigate: Callback<&str>,
    animate: Callback<Option<Sender<f64>>>,
) {
    animate(None);

    resolve(
        req,
        ShellOutput::Animate(AnimateOperationOutput::Done),
        &core,
        render,
        navigate,
        animate,
    );
}

fn start_animation(
    req: &mut Request<ShellOp>,
    core: &Core,
    render: WriteSignal<ViewModel>,
    navigate: Callback<&str>,
    animate: Callback<Option<Sender<f64>>>,
) {
    let (sx, mut rx) = channel::<f64>(1);
    let core = core.clone();
    log::info!("web: req start animation");

    spawn_local(async move {
        while let Some(ts) = rx.next().await {
            resolve(
                req,
                ShellOutput::Animate(AnimateOperationOutput::Timestamp(ts)),
                &core,
                render,
                navigate,
                animate,
            );
        }

        resolve(
            req,
            ShellOutput::Animate(AnimateOperationOutput::Done),
            &core,
            render,
            navigate,
            animate,
        );
    });

    animate(Some(sx));
}

fn audio_recording_premissions(
    req: &mut Request<ShellOp>,
    core: &Core,
    render: WriteSignal<ViewModel>,
    navigate: Callback<&str>,
    animate: Callback<Option<Sender<f64>>>,
) {
    #[cfg(feature = "browser")]
    {
        use js_sys::{Object, Reflect};
        use wasm_bindgen_futures::JsFuture;
        use web_sys::{window, MediaStreamConstraints, PermissionState, PermissionStatus};

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
                    PermissionState::Granted => PermissionsOperationOutput::Granted,
                    PermissionState::Denied => PermissionsOperationOutput::PermanentlyRejected,
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
                                Ok(_) => PermissionsOperationOutput::Granted,
                                Err(_) => PermissionsOperationOutput::Rejected,
                            }
                        } else {
                            PermissionsOperationOutput::Rejected
                        }
                    }
                    _ => PermissionsOperationOutput::Rejected,
                };

                resolve(
                    req,
                    ShellOutput::Permissions(grant),
                    &core,
                    render,
                    navigate,
                    animate,
                );
            })
        } else {
            resolve(
                req,
                ShellOutput::Permissions(PermissionsOperationOutput::Rejected),
                &core,
                render,
                navigate,
                animate,
            );
        }
    }
}
