pub use crux_core::App;
use crux_core::{render::Render, Capability};
use crux_kv::KeyValue;
use crux_macros::Effect;
use hecs::{Entity, World};
use serde::{Deserialize, Serialize};

mod objects;
mod play;
// mod node;
mod animate;
mod model;
mod visual;

pub use animate::*;
pub use play::*;
pub use visual::{VisualEV, VisualVM};

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Activity {
    #[default]
    Intro,
    Tune,
    Play,
    Listen,
    About,
    Unknown,
}

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct ViewModel {
    pub activity: Activity,
    pub visual: VisualVM,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum Event {
    InitialNavigation(Activity),
    Navigation(Activity),
    Visual(VisualEV),
    Resize(f64, f64),
    SafeAreaResize(f64, f64, f64, f64),
    PlayOpInstall(bool),
    PlayOpRun(PlayOperationOutput),
    PlayOpFftData(Vec<(f32, f32)>),
    PlayOpSnoopData(Vec<(Entity, Vec<f32>)>),
}

impl Eq for Event {}

#[derive(Default)]
pub struct RedSiren {
    visual: visual::Visual,
}

#[cfg_attr(feature = "typegen", derive(crux_macros::Export))]
#[derive(Effect)]
#[effect(app = "RedSiren")]
pub struct RedSirenCapabilities {
    pub render: Render<Event>,
    pub animate: Animate<Event>,
    pub play: Play<Event>,
}

impl From<&RedSirenCapabilities> for visual::VisualCapabilities {
    fn from(incoming: &RedSirenCapabilities) -> Self {
        Self {
            render: incoming.render.map_event(super::Event::Visual),
            animate: incoming.animate.map_event(super::Event::Visual),
        }
    }
}

impl App for RedSiren {
    type Event = Event;
    type Model = model::Model;
    type ViewModel = ViewModel;
    type Capabilities = RedSirenCapabilities;

    fn update(&self, msg: Event, model: &mut Self::Model, caps: &RedSirenCapabilities) {
        log::trace!("app msg: {:?}", msg);

        match msg {
            Event::InitialNavigation(activity) => {
                model.activity = activity;
                caps.play.install(Event::PlayOpInstall);
                self.visual
                    .update(VisualEV::AnimateEntrance, model, &caps.into());
                caps.render.render();
            }
            Event::PlayOpInstall(success) => {
                if success {
                    caps.play.run_unit(
                        Event::PlayOpRun,
                        Event::PlayOpFftData,
                        Event::PlayOpSnoopData,
                    )
                }
                // PlayOperationOutput::Success => ,
                // PlayOperationOutput::Failure => {
                //     caps.play.install(Event::PlayOpInstall);
                //     log::error!("failed to instal audio unit, retrying");
                // }
                // PlayOperationOutput::PermanentFailure => {
                //     log::error!("permanently failed to instal audio unit");
                // }
            },
            Event::PlayOpRun(success) => match success {
                PlayOperationOutput::Success => {
                    log::info!("running");
                }
                PlayOperationOutput::Failure => {
                    caps.play.install(Event::PlayOpInstall);
                    log::error!("failed to instal audio unit, retrying");
                }
                PlayOperationOutput::PermanentFailure => {
                    log::error!("permanently failed to instal audio unit");
                }
            },
            Event::PlayOpFftData(d) => log::info!("fft data"),
            Event::PlayOpSnoopData(d) => log::info!("snoop data"),
            Event::Navigation(activity) => {
                model.activity = activity;
                caps.render.render();
            }
            Event::Resize(width, height) => {
                self.visual.resize(width, height, model);
                caps.render.render();
            }
            Event::SafeAreaResize(left, top, right, bottom) => {
                self.visual.safe_area(left, top, right, bottom, model);
                caps.render.render();
            }
            Event::Visual(ev) => self.visual.update(ev, model, &caps.into()),
        }
    }

    fn view(&self, model: &Self::Model) -> ViewModel {
        ViewModel {
            activity: model.activity,
            visual: self.visual.view(&model),
        }
    }
}

#[cfg(test)]
mod tests {}
