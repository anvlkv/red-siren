mod app;
mod au;
mod shell;

use std::collections::HashMap;

use ::shared::*;
use app_core::{
    Animate, AnimateOperationOutput, Core, Permissions, PermissionsOperationOutput, RedSiren,
    RedSirenCapabilities, ViewModel,
};
use crux_core::{render::Render, App};
use crux_macros::Effect;
use crux_platform::{Platform, PlatformResponse};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use app::*;
pub use au::*;
pub use shell::*;

#[derive(Default)]
pub struct RedSirenBridge;

#[derive(Default)]
pub struct Model {
    view: ViewModel,
    platform: Option<PlatformKind>,
}

#[cfg_attr(feature = "typegen", derive(crux_macros::Export))]
#[derive(Effect)]
#[effect(app = "RedSirenBridge")]
pub struct RedSirenBridgeCapabilities {
    render: Render<Event>,
    platform: Platform<Event>,
    shell: ShellCapability<Event>,
    #[effect(skip)]
    app: AppCapability<Event>,
    #[effect(skip)]
    au: AUCapability<Event>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum Event {
    Start,
    SetPlatform(PlatformResponse),
    AppEvent(app_core::Event),
    #[serde(skip)]
    AppEffect(app_core::Effect),
    #[serde(skip)]
    AppView(app_core::ViewModel),
    #[serde(skip)]
    AuEvent(au_core::UnitEvent),
    #[serde(skip)]
    AuEffect(au_core::Effect),
}

impl App for RedSirenBridge {
    type Event = Event;

    type Model = Model;

    type ViewModel = ViewModel;

    type Capabilities = RedSirenBridgeCapabilities;

    fn update(&self, event: Self::Event, model: &mut Self::Model, caps: &Self::Capabilities) {
        match event {
            Event::Start => {
                caps.platform.get(Event::SetPlatform);
                caps.app.receive_view(Event::AppView);
                caps.app.receive_effects(Event::AppEffect);
                caps.au.receive_effects(Event::AuEffect);
            }
            Event::SetPlatform(platform) => {
                let kind: PlatformKind = platform.0.into();
                model.platform = Some(kind);
                caps.au.forward_event(UnitEvent::SetPlatform(kind))
            }
            Event::AppView(view) => {
                model.view = view;
                caps.render.render();
            }
            Event::AppEvent(ev) => caps.app.forward_event(ev),
            Event::AuEvent(ev) => caps.au.forward_event(ev),
            Event::AppEffect(eff) => match eff {
                app_core::Effect::Animate(req) => caps.shell.resolve(&req.operation, {
                    let app = caps.app.clone();
                    let mut req = req;
                    |op: AnimateOperationOutput| {
                        app.resolve(&mut req, op);
                    }
                }),
                app_core::Effect::Permissions(req) => caps.shell.resolve(&req.operation, {
                    let app = caps.app.clone();
                    let mut req = req;
                    |op: PermissionsOperationOutput| {
                        app.resolve(&mut req, op);
                    }
                }),
                app_core::Effect::Play(req) => caps.au.make_request(&req.operation, {
                    let app = caps.app.clone();
                    let mut req = req;
                    move |output| {
                        app.resolve(output, &mut req);
                    }
                }),
                app_core::Effect::Render(_) => caps.render.render(),
            },
            Event::AuEffect(eff) => match eff {
                au_core::Effect::Resolve(req) => caps.app.make_request(&req.operation, {
                    let au = caps.au.clone();
                    move |output| {
                        au.resolve(output, req);
                    }
                }),
                au_core::Effect::SystemCapability(mut req) => {}
            },
        }
    }

    fn view(&self, model: &Self::Model) -> Self::ViewModel {
        model.view.clone()
    }
}

impl RedSirenBridge {
    fn stream_params() -> (u32, u32) {
        cfg_if::cfg_if! {
            if #[cfg(target_arch = "wasm32")] {
                (44100, 128)
            } else {
                cpal::traits::HostTrait::default_output_device(&cpal::default_host())
                    .map(|d| cpal::traits::DeviceTrait::default_output_config(&d).ok())
                    .flatten()
                    .map(|d| {
                        (
                            d.sample_rate().0,
                            match d.buffer_size() {
                                cpal::SupportedBufferSize::Range { min, max } => {
                                    let non_zero = Ord::max(*min, DESIRED_BUFFER_SIZE);
                                    if *max != 0 {
                                        Ord::min(non_zero, *max)
                                    } else {
                                        non_zero
                                    }
                                }
                                cpal::SupportedBufferSize::Unknown => DESIRED_BUFFER_SIZE,
                            },
                        )
                    })
                    .unwrap_or((44100, DESIRED_BUFFER_SIZE))
            }
        }
    }
}
