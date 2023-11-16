use serde::{Deserialize, Serialize};

use crux_core::render::Render;
use crux_core::App;
use crux_kv::KeyValue;
use crux_macros::Effect;


#[derive(Default)]
pub struct Tuner;

#[derive(Default, Serialize, Deserialize, Clone)]
pub struct Model {

}

#[derive(Default, Serialize, Deserialize, Clone)]
pub struct TunerVM {

}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum TunerEV {
  GetTuning,
  SetTuning,

}

#[cfg_attr(feature = "typegen", derive(crux_macros::Export))]
#[derive(Effect)]
#[effect(app = "Tuner")]
pub struct TunerCapabilities {
    pub render: Render<TunerEV>,
    pub key_value: KeyValue<TunerEV>,
}

impl App for Tuner {
    type Event = TunerEV;

    type Model = Model;

    type ViewModel = TunerVM;

    type Capabilities = TunerCapabilities;

    fn update(&self, event: Self::Event, model: &mut Self::Model, caps: &Self::Capabilities) {
        todo!()
    }

    fn view(&self, model: &Self::Model) -> Self::ViewModel {
        TunerVM::default()
    }
}