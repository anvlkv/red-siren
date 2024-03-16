use std::sync::Arc;

use crux_core::Core;

use wasm_bindgen::prelude::wasm_bindgen;

use crate::{AUCapabilities, Effect, FFTData, RedSirenAU, SnoopsData, UnitEvent};

lazy_static::lazy_static! {
  static ref AU_UNIT: Arc<Core<Effect, RedSirenAU>> = Arc::new(Core::new::<AUCapabilities>());
}

#[wasm_bindgen]
pub fn init_once() {
    _ = console_log::init_with_level(log::Level::Warn);
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub fn instantiate_unit() {
    _ = Core::process_event(&AU_UNIT, UnitEvent::RunUnit);
}

#[derive(Default)]
#[wasm_bindgen(getter_with_clone)]
pub struct WorkletModel {
    pub samples_l: Vec<f32>,
    pub samples_r: Vec<f32>,
    pub fft_data: Option<Vec<u8>>,
    pub snoops_data: Option<Vec<u8>>,
    pub run_unit: Option<bool>,
    pub update_unit: Option<bool>,
}

impl From<Vec<Effect>> for WorkletModel {
    fn from(effects: Vec<Effect>) -> Self {
        let mut model = Self::default();

        for effect in effects {
            match effect {
                Effect::Render(_) => todo!(),
                Effect::Resolve(req) => match req.operation {
                    crate::UnitResolve::RunUnit(success) => model.run_unit = Some(success),
                    crate::UnitResolve::UpdateEV(success) => model.update_unit = Some(success),
                    crate::UnitResolve::FftData(data) => model.fft_data = Some(data),
                    crate::UnitResolve::SnoopsData(data) => model.snoops_data = Some(data),
                },
            }
        }

        model
    }
}

#[wasm_bindgen]
pub fn process_samples(samples: &[f32]) -> Vec<f32> {}

#[wasm_bindgen]
pub fn unit_update(update: &[u8]) -> WorkletModel {
    let ev = bincode::deserialize::<UnitEvent>(update).expect("deserialize");
    _ = Core::process_event(&AU_UNIT, ev);
}
