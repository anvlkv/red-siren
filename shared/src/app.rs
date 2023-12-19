pub use crux_core::App;
use crux_core::{render::Render, Capability};
use crux_kv::KeyValue;
use crux_macros::Effect;
use serde::{Deserialize, Serialize};

use crate::animate::Animate;
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

#[derive(Default)]
pub struct Model {
    instrument: instrument::Model,
    tuning: tuner::Model,
    intro: intro::Model,
    activity: Activity,
    config: Option<instrument::Config>,
}

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct ViewModel {
    pub activity: Activity,
    pub intro: intro::IntroVM,
    pub tuning: tuner::TunerVM,
    pub instrument: instrument::InstrumentVM,
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
        log::trace!("msg: {:?}", msg);

        match msg {
            Event::Start => {
                caps.render.render();
            }
            Event::ReflectActivity(act) => {
                model.activity = act;
                model.intro.current_activity = act;
                caps.render.render();
            }
            Event::Menu(act) => match (model.activity, act) {
                (Activity::Intro, Activity::Play) => {
                    self.intro
                        .update(intro::IntroEV::Menu(act), &mut model.intro, &caps.into());
                    self.instrument.update(
                        instrument::InstrumentEV::Playback(instrument::PlaybackEV::Play(true)),
                        &mut model.instrument,
                        &caps.into(),
                    );
                }
                (Activity::Intro, Activity::Tune) => {
                    self.intro
                        .update(intro::IntroEV::Menu(act), &mut model.intro, &caps.into());
                }
                (Activity::Intro, Activity::About) => {
                    self.intro
                        .update(intro::IntroEV::Menu(act), &mut model.intro, &caps.into());
                }
                (Activity::About, Activity::Intro) => {
                    self.intro
                        .update(intro::IntroEV::Menu(act), &mut model.intro, &caps.into());
                }
                (Activity::Play, Activity::Tune) => {
                    self.instrument.update(
                        instrument::InstrumentEV::Playback(instrument::PlaybackEV::Play(false)),
                        &mut model.instrument,
                        &caps.into(),
                    );
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
                _ => todo!("transition not implemented"),
            },
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
                    instrument::InstrumentEV::CreateWithConfig(config),
                    &mut model.instrument,
                    &caps.into(),
                );
                self.intro.update(
                    intro::IntroEV::SetInstrumentTarget(
                        Box::new(model.instrument.layout.as_ref().unwrap().clone()),
                        Box::new(model.instrument.config.clone()),
                    ),
                    &mut model.intro,
                    &caps.into(),
                );
                _ = model.config.insert(model.instrument.config.clone());
            }
            Event::InstrumentEvent(event) => {
                self.instrument
                    .update(event, &mut model.instrument, &caps.into())
            }
            Event::TunerEvent(event) => self.tuner.update(event, &mut model.tuning, &caps.into()),
            Event::IntroEvent(event) => self.intro.update(event, &mut model.intro, &caps.into()),
        }
    }

    fn view(&self, model: &Model) -> ViewModel {
        ViewModel {
            activity: model.activity,
            tuning: self.tuner.view(&model.tuning),
            intro: self.intro.view(&model.intro),
            instrument: self.instrument.view(&model.instrument),
        }
    }
}

#[cfg(test)]
mod tests {}
