use std::sync::{mpsc::sync_channel, Arc, Mutex};

use au_core::{Unit, UnitResolve};
pub use crux_core::App;
use crux_core::{render::Render, Capability};
use crux_macros::Effect;
use futures::channel::mpsc::unbounded;
use hecs::Entity;
use serde::{Deserialize, Serialize};

mod animate;
mod config;
mod instrument;
mod layout;
mod model;
mod objects;
mod paint;
mod play;
mod visual;

pub use animate::*;
pub use objects::*;
pub use paint::*;
pub use play::*;
pub use visual::*;

use crate::app::{config::Config, instrument::Instrument, layout::Layout};

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

    // PlayOpInstall(bool),
    PlayOpResolve(UnitResolve),
    PlayOpFftData(Vec<(f32, f32)>),
    PlayOpSnoopData(Vec<(Entity, Vec<f32>)>),
    StartAudioUnit,
}

impl Eq for Event {}

#[derive(Default)]
pub struct RedSiren {
    visual: visual::Visual,
    audio_unit: Arc<Mutex<Option<Unit>>>,
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
                self.visual
                    .update(VisualEV::AnimateEntrance, model, &caps.into());
                caps.render.render();
                let (unit_resolve_sender, unit_resolve_receiver) = unbounded();
                let unit = Unit::new(unit_resolve_sender);
                _ = self.audio_unit.lock().unwrap().insert(unit);
                caps.play.with_receiver(unit_resolve_receiver);
            }
            Event::StartAudioUnit => {
                let mut unit = self.audio_unit.lock().unwrap();
                let unit = unit.as_mut().unwrap();

                let (fft_sender, fft_receiver) = sync_channel::<Vec<(f32, f32)>>(4);
                let (snoops_sender, snoops_receiver) = sync_channel::<Vec<(Entity, Vec<f32>)>>(8);

                caps.play.run_unit(Event::PlayOpResolve);

                unit.run(fft_sender, snoops_sender).expect("run unit");

                caps.animate
                    .animate_reception(Event::PlayOpSnoopData, snoops_receiver);
                caps.animate
                    .animate_reception(Event::PlayOpFftData, fft_receiver);
            }
            Event::PlayOpResolve(unit_resolve) => match unit_resolve {
                UnitResolve::RunUnit(true) => {
                    let mut unit = self.audio_unit.lock().unwrap();
                    let unit = unit.as_mut().unwrap();
                    let world = model.world.lock().unwrap();
                    unit.update(au_core::UnitEV::Configure(
                        model.instrument.get_nodes(&world),
                    ));
                }
                UnitResolve::RunUnit(false) => {
                    log::error!("run unit error");
                }
                UnitResolve::UpdateEV(true) => {
                    log::info!("updated unit");
                }
                UnitResolve::UpdateEV(false) => {
                    log::error!("update unit error");
                }
            },
            // Event::PlayOpInstall(success) => {
            //     if success {
            //         caps.play.run_unit(
            //             Event::PlayOpRun,
            //             Event::PlayOpFftData,
            //             Event::PlayOpSnoopData,
            //         )
            //     }
            //     // PlayOperationOutput::Success => ,
            //     // PlayOperationOutput::Failure => {
            //     //     caps.play.install(Event::PlayOpInstall);
            //     //     log::error!("failed to instal audio unit, retrying");
            //     // }
            //     // PlayOperationOutput::PermanentFailure => {
            //     //     log::error!("permanently failed to instal audio unit");
            //     // }
            // },
            // Event::PlayOpRun(success) => match success {
            //     PlayOperationOutput::Success => {
            //         log::info!("running");
            //     }
            //     PlayOperationOutput::Failure => {
            //         caps.play.install(Event::PlayOpInstall);
            //         log::error!("failed to instal audio unit, retrying");
            //     }
            //     PlayOperationOutput::PermanentFailure => {
            //         log::error!("permanently failed to instal audio unit");
            //     }
            // },
            Event::PlayOpFftData(d) => log::info!("fft data"),
            Event::PlayOpSnoopData(d) => log::info!("snoop data"),
            Event::Navigation(activity) => {
                model.activity = activity;
                caps.render.render();
            }
            Event::Visual(ev) => match ev {
                VisualEV::Resize(_, _)
                | VisualEV::SafeAreaResize(_, _, _, _)
                | VisualEV::SetDensity(_) => {
                    self.visual.update(ev, model, &caps.into());
                    if model.density > 0_f64 {
                        model.configs = Config::configs_for_screen(
                            model.view_box.width(),
                            model.view_box.height(),
                            model.density,
                            [
                                model.safe_area.left,
                                model.safe_area.top,
                                model.safe_area.right,
                                model.safe_area.bottom,
                            ],
                        );
                        log::debug!("add configs: {}", model.configs.len());
                        if model.current_config >= model.configs.len() {
                            model.current_config = 0;
                        }

                        let config = model.configs.get(model.current_config).unwrap();

                        {
                            let mut world = model.world.lock().unwrap();
                            world.clear();

                            model.layout = Layout::layout(config, &mut world).unwrap();

                            model.instrument =
                                Instrument::new(config, &mut world, &model.layout).unwrap();

                            model.objects =
                                Objects::new(&mut world, &model.layout, model.dark_schema)
                        }

                        self.visual
                            .update(VisualEV::LayoutUpdate, model, &caps.into());
                    }
                }
                _ => {
                    self.visual.update(ev, model, &caps.into());
                }
            },
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
