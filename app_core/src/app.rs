use std::sync::{Arc, Mutex};

pub use crux_core::App;
use crux_core::{render::Render, Capability};
use crux_kv::KeyValue;
use crux_macros::Effect;
use hecs::World;
use serde::{Deserialize, Serialize};

use crate::{animate::Animate, geometry::Rect};
pub use instrument::Instrument;
pub use intro::Intro;
pub use navigate::Navigate;
pub use play::Play;
pub use tuner::Tuner;

use self::{
    instrument::InstrumentCapabilities, intro::IntroCapabilities, tuner::TunerCapabilities,
};

pub mod animate;
pub mod instrument;
pub mod intro;
pub mod navigate;
pub mod play;
pub mod tuner;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Activity {
    #[default]
    Intro,
    Tune,
    Play,
    Listen,
    About,
}

pub struct Model {
    instrument: instrument::Model,
    tuner: tuner::Model,
    intro: intro::Model,
    activity: Activity,
    _world: Arc<Mutex<World>>,
    config: Option<instrument::Config>,
    view_box: Rect,
}

impl Default for Model {
    fn default() -> Self {
        let world = Arc::new(Mutex::new(World::new()));
        Self {
            instrument: instrument::Model::new(world.clone()),
            tuner: tuner::Model::new(world.clone()),
            _world: world.clone(),
            intro: Default::default(),
            activity: Default::default(),
            view_box: Default::default(),
            config: None,
        }
    }
}

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct ViewModel {
    pub activity: Activity,
    pub intro: intro::IntroVM,
    pub tuner: tuner::TunerVM,
    pub instrument: instrument::InstrumentVM,
    pub view_box: Rect,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum Event {
    Start,
    TunerEvent(tuner::TunerEV),
    InstrumentEvent(instrument::InstrumentEV),
    IntroEvent(intro::IntroEV),
    ConfigureApp(instrument::Config),
    CreateConfigAndConfigureApp {
        width: f64,
        height: f64,
        dpi: f64,
        safe_areas: [f64; 4],
    },
    ReflectActivity(Activity),
    Menu(Activity),
    Capture(play::CaptureOutput),
}

impl Eq for Event {}

#[derive(Default)]
pub struct RedSiren {
    pub tuner: Tuner,
    pub instrument: Instrument,
    pub intro: Intro,
}

#[cfg_attr(feature = "typegen", derive(crux_macros::Export))]
#[derive(Effect)]
#[effect(app = "RedSiren")]
pub struct RedSirenCapabilities {
    pub render: Render<Event>,
    pub key_value: KeyValue<Event>,
    pub navigate: Navigate<Event>,
    pub play: Play<Event>,
    pub animate: Animate<Event>,
}

impl From<&RedSirenCapabilities> for IntroCapabilities {
    fn from(incoming: &RedSirenCapabilities) -> Self {
        IntroCapabilities {
            render: incoming.render.map_event(super::Event::IntroEvent),
            navigate: incoming.navigate.map_event(super::Event::IntroEvent),
            animate: incoming.animate.map_event(super::Event::IntroEvent),
        }
    }
}

impl From<&RedSirenCapabilities> for TunerCapabilities {
    fn from(incoming: &RedSirenCapabilities) -> Self {
        TunerCapabilities {
            key_value: incoming.key_value.map_event(super::Event::TunerEvent),
            render: incoming.render.map_event(super::Event::TunerEvent),
            play: incoming.play.map_event(super::Event::TunerEvent),
            navigate: incoming.navigate.map_event(super::Event::TunerEvent),
        }
    }
}

impl From<&RedSirenCapabilities> for InstrumentCapabilities {
    fn from(incoming: &RedSirenCapabilities) -> Self {
        InstrumentCapabilities {
            render: incoming.render.map_event(super::Event::InstrumentEvent),
            play: incoming.play.map_event(super::Event::InstrumentEvent),
            navigate: incoming.navigate.map_event(super::Event::InstrumentEvent),
        }
    }
}

impl App for RedSiren {
    type Event = Event;
    type Model = Model;
    type ViewModel = ViewModel;
    type Capabilities = RedSirenCapabilities;

    fn update(&self, msg: Event, model: &mut Model, caps: &RedSirenCapabilities) {
        log::trace!("app msg: {:?}", msg);

        match msg {
            Event::Start => {
                self.tuner.update(
                    tuner::TunerEV::CheckHasTuning,
                    &mut model.tuner,
                    &caps.into(),
                );
                caps.render.render();
            }
            Event::ReflectActivity(act) => {
                model.activity = act;
                model.intro.current_activity = act;
                caps.render.render();

                log::debug!("reflect {act:?}");

                if act == Activity::Play {
                    if !self.tuner.is_tuned(&model.tuner) {
                        self.update(Event::Menu(Activity::Tune), model, caps);
                    } else if let Some(d) = model.tuner.tuning.as_ref() {
                        model.instrument.tuning = d.clone();
                    }
                } else if act == Activity::Tune {
                    model.tuner.state = if model.instrument.setup_complete {
                        tuner::State::SetupComplete
                    } else {
                        tuner::State::None
                    };
                    self.tuner.update(
                        tuner::TunerEV::Activate(true),
                        &mut model.tuner,
                        &caps.into(),
                    );
                } else if act == Activity::Intro && model.activity != act {
                    self.intro.update(
                        intro::IntroEV::Menu(act),
                        &mut model.intro,
                        &caps.into(),
                    );
                }
            }
            Event::Menu(act) => {
                log::debug!("menu {act:?}");
                match (model.activity, act) {
                    (Activity::Intro, Activity::Play) => {
                        if !self.tuner.is_tuned(&model.tuner) {
                            self.update(Event::Menu(Activity::Tune), model, caps);
                        } else {
                            self.intro.update(
                                intro::IntroEV::Menu(act),
                                &mut model.intro,
                                &caps.into(),
                            );
                            self.instrument.update(
                                instrument::InstrumentEV::Playback(instrument::PlaybackEV::Play(
                                    true,
                                )),
                                &mut model.instrument,
                                &caps.into(),
                            );
                        }
                    }
                    (Activity::Intro, Activity::Tune) => {
                        model.tuner.state = if model.instrument.setup_complete {
                            tuner::State::SetupComplete
                        } else {
                            tuner::State::None
                        };
                        self.intro.update(
                            intro::IntroEV::Menu(act),
                            &mut model.intro,
                            &caps.into(),
                        );
                    }
                    (Activity::Intro, Activity::About) => {
                        self.intro.update(
                            intro::IntroEV::Menu(act),
                            &mut model.intro,
                            &caps.into(),
                        );
                    }
                    (Activity::About, Activity::Intro) => {
                        self.intro.update(
                            intro::IntroEV::Menu(act),
                            &mut model.intro,
                            &caps.into(),
                        );
                    }
                    (Activity::Play, Activity::Tune) => {
                        model.tuner.state = if model.instrument.setup_complete {
                            tuner::State::SetupComplete
                        } else {
                            tuner::State::None
                        };
                        self.instrument.update(
                            instrument::InstrumentEV::Playback(instrument::PlaybackEV::Play(false)),
                            &mut model.instrument,
                            &caps.into(),
                        );
                        self.intro.update(
                            intro::IntroEV::Menu(act),
                            &mut model.intro,
                            &caps.into(),
                        );
                        self.update(Event::ReflectActivity(Activity::Intro), model, caps);
                    }
                    (Activity::Play, Activity::Play) => {
                        self.instrument.update(
                            instrument::InstrumentEV::Playback(instrument::PlaybackEV::Play(
                                !model.instrument.playing,
                            )),
                            &mut model.instrument,
                            &caps.into(),
                        );
                    }
                    (Activity::Tune, _) => {
                        self.tuner.update(
                            tuner::TunerEV::Activate(false),
                            &mut model.tuner,
                            &caps.into(),
                        );
                        if let Some(tuning) = model.tuner.tuning.clone() {
                            model.instrument.setup_complete =
                                model.tuner.state >= tuner::State::SetupComplete;
                            model.instrument.tuning = tuning;
                            model.instrument.configured = false;
                            self.intro.update(
                                intro::IntroEV::Menu(act),
                                &mut model.intro,
                                &caps.into(),
                            );
                            self.update(Event::ReflectActivity(Activity::Intro), model, caps);
                        } else {
                            log::warn!("leaving tuner without complete tuning");
                            self.tuner.update(
                                tuner::TunerEV::Activate(true),
                                &mut model.tuner,
                                &caps.into(),
                            );
                        }
                    }
                    _ => todo!("transition not implemented"),
                }
            }
            Event::CreateConfigAndConfigureApp {
                width,
                height,
                dpi,
                safe_areas,
            } => {
                let config = instrument::Config::new(width, height, dpi, safe_areas);
                self.update(Event::ConfigureApp(config), model, caps);
            }
            Event::ConfigureApp(config) => {
                self.instrument.update(
                    instrument::InstrumentEV::CreateWithConfig(config.clone()),
                    &mut model.instrument,
                    &caps.into(),
                );
                self.tuner.update(
                    tuner::TunerEV::SetConfig(config.clone()),
                    &mut model.tuner,
                    &caps.into(),
                );
                self.intro.update(
                    intro::IntroEV::SetInstrumentTarget(
                        Box::new(model.instrument.layout.as_ref().unwrap().clone()),
                        Box::new(config.clone()),
                    ),
                    &mut model.intro,
                    &caps.into(),
                );
                model.view_box = Rect::size(config.width, config.height);
                _ = model.config.insert(config);
            }
            Event::InstrumentEvent(event) => {
                self.instrument
                    .update(event, &mut model.instrument, &caps.into());
            }
            Event::TunerEvent(event) => {
                self.tuner.update(event, &mut model.tuner, &caps.into());
            }
            Event::Capture(ev) => match ev {
                play::CaptureOutput::CaptureFFT(d) => {
                    self.tuner
                        .update(tuner::TunerEV::FftData(d), &mut model.tuner, &caps.into())
                }
                play::CaptureOutput::CaptureData(d) => {
                    self.instrument.update(
                        instrument::InstrumentEV::SnoopData(d),
                        &mut model.instrument,
                        &caps.into(),
                    );
                }
                play::CaptureOutput::CaptureNodesData(d) => {
                    self.instrument.update(
                        instrument::InstrumentEV::NodeSnoopData(d),
                        &mut model.instrument,
                        &caps.into(),
                    );
                }

            },
            Event::IntroEvent(event) => self.intro.update(event, &mut model.intro, &caps.into()),
        }
    }

    fn view(&self, model: &Model) -> ViewModel {
        ViewModel {
            activity: model.activity,
            tuner: self.tuner.view(&model.tuner),
            intro: self.intro.view(&model.intro),
            instrument: self.instrument.view(&model.instrument),
            view_box: model.view_box,
        }
    }
}

#[cfg(test)]
mod tests {}
