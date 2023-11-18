pub use crux_core::App;
use crux_core::{render::Render, Capability};
use crux_kv::KeyValue;
use crux_macros::Effect;
use serde::{Deserialize, Serialize};

pub mod navigate;
pub mod instrument;
pub mod intro;
pub mod tuner;

pub use navigate::Navigate;
pub use instrument::Instrument;
pub use intro::Intro;
pub use tuner::Tuner;

use self::{
    instrument::InstrumentCapabilities, intro::IntroCapabilities, tuner::TunerCapabilities,
};

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Activity {
    Intro,
    Tune,
    Play,
    Listen,
}

impl Default for Activity {
    fn default() -> Self {
        Activity::Intro
    }
}

#[derive(Default)]
pub struct Model {
    instrument: instrument::Model,
    tuning: tuner::Model,
    intro: intro::Model,
    activity: Activity,
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
    None,
    TunerEvent(tuner::TunerEV),
    InstrumentEvent(instrument::InstrumentEV),
    IntroEvent(intro::IntroEV),
    ConfigureApp(instrument::Config),
    CreateConfigAndConfigureApp(f32, f32, f32),
    Activate(Activity)
}

impl Eq for Event{}

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
    pub navigate: Navigate<Event>
}


impl From<&RedSirenCapabilities> for IntroCapabilities {
    fn from(incoming: &RedSirenCapabilities) -> Self {
        IntroCapabilities {
            render: incoming.render.map_event(super::Event::IntroEvent),
            navigate: incoming.navigate.map_event(super::Event::IntroEvent)
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
            key_value: incoming.key_value.map_event(super::Event::InstrumentEvent),
            render: incoming.render.map_event(super::Event::InstrumentEvent),
        }
    }
}

impl App for RedSiren {
    type Model = Model;
    type Event = Event;
    type ViewModel = ViewModel;
    type Capabilities = RedSirenCapabilities;

    fn update(&self, msg: Event, model: &mut Model, caps: &RedSirenCapabilities) {
        match msg {
            Event::None => {
                caps.render.render();
            }
            Event::Activate(act) => {
                model.activity = act;
                caps.render.render();
            }
            Event::CreateConfigAndConfigureApp(width, height, density) => {
                let config = instrument::Config::new(width, height, density);
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
                        model.instrument.layout.as_ref().unwrap().clone(),
                        model.instrument.config.clone(),
                    ),
                    &mut model.intro,
                    &caps.into(),
                );
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
