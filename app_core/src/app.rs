pub use au_core::{FFTData, SnoopsData, UnitResolve, UnitState};
use au_core::{Unit, UnitEV};
pub use crux_core::App;
use crux_core::{render::Render, Capability};
use crux_macros::Effect;
use futures::channel::mpsc::unbounded;
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
    pub unit_state: UnitState,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum Event {
    InitialNavigation(Activity),
    Navigation(Activity),
    Visual(VisualEV),
    PlayOpResolve(UnitResolve),
    PlayOpFftData(FFTData),
    PlayOpSnoopData(SnoopsData),
    StartAudioUnit,
    Pause,
    Resume,
    AnimationStopped(),
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
                self.visual
                    .update(VisualEV::AnimateEntrance, model, &caps.into());
                caps.render.render();
                let (unit_resolve_sender, unit_resolve_receiver) = unbounded();
                caps.play.with_receiver(unit_resolve_receiver);
                let unit = Unit::new(unit_resolve_sender);
                _ = model.audio_unit.lock().insert(unit);
            }
            Event::StartAudioUnit => {
                caps.play.recording_permission(Event::PlayOpResolve);
            }
            Event::Pause => {
                caps.animate.stop(Event::AnimationStopped);
            }
            Event::AnimationStopped() => {
                {
                    let mut unit = model.audio_unit.lock();
                    let unit = unit.as_mut().unwrap();
                    unit.update(UnitEV::Suspend);
                }
                self.visual
                    .update(VisualEV::ClearSnoops, model, &caps.into());
            }
            Event::Resume => {
                let app_au_buffer = {
                    let mut unit = model.audio_unit.lock();
                    let unit = unit.as_mut().unwrap();
                    unit.update(UnitEV::Resume);
                    unit.app_au_buffer.clone()
                };
                caps.animate.animate_reception(
                    Event::PlayOpSnoopData,
                    move || app_au_buffer.read_snoops_data(),
                    "snoops",
                );
                caps.render.render();
                // caps.animate
                //     .animate_reception(Event::PlayOpFftData, fft_cons(), "fft");
            }
            Event::PlayOpResolve(unit_resolve) => match unit_resolve {
                UnitResolve::RecordingPermission(true) => {
                    let mut unit = model.audio_unit.lock();
                    let unit = unit.as_mut().unwrap();

                    caps.play.run_unit(Event::PlayOpResolve);

                    unit.run().expect("run unit");

                    let app_au_buffer = unit.app_au_buffer.clone();
                    caps.animate.animate_reception(
                        Event::PlayOpSnoopData,
                        move || app_au_buffer.read_snoops_data(),
                        "snoops",
                    );

                    // caps.animate
                    //     .animate_reception(Event::PlayOpFftData, fft_cons(), "fft");

                    log::info!("started unit and animate reception");
                }
                UnitResolve::RecordingPermission(false) => {
                    log::error!("no recording permission");
                }
                UnitResolve::RunUnit(true) => {
                    log::info!("unit running, configure");
                    let mut unit = model.audio_unit.lock();
                    let unit = unit.as_mut().unwrap();
                    let world = model.world.lock();
                    unit.update(au_core::UnitEV::Configure(
                        model.instrument.get_nodes(&world),
                    ));
                    unit.update(au_core::UnitEV::Resume);
                }
                UnitResolve::RunUnit(false) => {
                    log::error!("run unit error");
                }
                UnitResolve::UpdateEV(true) => {
                    caps.render.render();
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
                            let mut world = model.world.lock();
                            world.clear();

                            model.layout = Layout::new(&config, &mut world).unwrap();

                            model.instrument =
                                Instrument::new(&config, &mut world, &model.layout).unwrap();

                            model.objects =
                                model.layout.make_objects(&mut world, model.dark_schema);
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
        let unit_state = model
            .audio_unit
            .lock()
            .as_ref()
            .map(|u| *u.state.lock())
            .unwrap_or_default();

        ViewModel {
            activity: model.activity,
            visual: self.visual.view(model),
            unit_state,
        }
    }
}

#[cfg(test)]
mod tests {}
