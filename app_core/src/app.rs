use std::sync::{Arc, Mutex};

use au_core::{Unit, UnitEV, UnitResolve, FFT_BUF_SIZE, SNOOPS_BUF_SIZE};
pub use crux_core::App;
use crux_core::{render::Render, Capability};
use crux_macros::Effect;
use futures::channel::mpsc::unbounded;
use hecs::Entity;
use once_mut::once_mut;
use ringbuf::StaticRb;
use serde::{Deserialize, Serialize};

once_mut! {
    static mut FFT_RB: StaticRb::<Vec<(f32, f32)>, FFT_BUF_SIZE> = StaticRb::default();
    static mut SNOOPS_RB: StaticRb::<Vec<(Entity, Vec<f32>)>, SNOOPS_BUF_SIZE> = StaticRb::default();
}

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
    Pause,
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

                let (fft_prod, fft_cons) = FFT_RB.take().unwrap().split_ref();
                let (snoops_prod, snoops_cons) = SNOOPS_RB.take().unwrap().split_ref();

                caps.play.run_unit(Event::PlayOpResolve);

                unit.run(fft_prod, snoops_prod).expect("run unit");

                caps.animate
                    .animate_reception(Event::PlayOpSnoopData, snoops_cons, "snoops");
                caps.animate
                    .animate_reception(Event::PlayOpFftData, fft_cons, "fft");

                log::info!("started unit and animate reception");
            }
            Event::Pause => {
                let mut unit = self.audio_unit.lock().unwrap();
                let unit = unit.as_mut().unwrap();
                unit.update(UnitEV::Suspend);

                caps.render.render();
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
            Event::PlayOpFftData(d) => log::info!("fft data"),
            Event::PlayOpSnoopData(d) => {
                self.visual
                    .update(VisualEV::SnoopsData(d), model, &caps.into());
            }
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

                        if let Some(config) = model.get_config().cloned() {
                            let mut world = model.world.lock().unwrap();
                            world.clear();

                            model.layout = Layout::layout(&config, &mut world).unwrap();

                            model.instrument =
                                Instrument::new(&config, &mut world, &model.layout).unwrap();

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
