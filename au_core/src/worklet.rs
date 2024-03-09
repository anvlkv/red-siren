use std::sync::Arc;

use itertools::interleave;
use parking_lot::Mutex;
use wasm_bindgen::prelude::wasm_bindgen;

use crate::{FFTData, SnoopsData, Unit, UnitEV};

lazy_static::lazy_static! {
  static ref AU_UNIT: Arc<Mutex<Option<Unit>>> = Default::default();
}

thread_local! {
    static CALLBACK: Arc<Mutex<Box<dyn FnMut(&[f32]) -> [[f32; 128];2]>>> = Arc::new(Mutex::new(Box::new(|_| [[0_f32; 128];2])));
}

#[wasm_bindgen]
pub fn init_once() {
    _ = console_log::init_with_level(log::Level::Warn);
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub fn instantiate_unit() {
    CALLBACK.with(|callback| {
        let mut unit = Unit::new();
        unit.run(callback.as_ref()).expect("run unit");
        _ = AU_UNIT.lock().insert(unit);
    })
}

#[wasm_bindgen]
pub fn process_samples(samples: &[f32]) -> Vec<f32> {
    CALLBACK.with(|callback| {
        let mut callback = callback.lock();

        let [ch1, ch2] = callback(samples);

        log::info!("callback tick");

        interleave(ch1, ch2).collect()
    })
}

#[wasm_bindgen]
pub fn get_fft_data() -> Option<Vec<u8>> {
    let unit = AU_UNIT.lock();
    unit.as_ref()
        .unwrap()
        .next_fft_reading()
        .map(|result| bincode::serialize::<FFTData>(&result).expect("serialize"))
}

#[wasm_bindgen]
pub fn get_snoops_data() -> Option<Vec<u8>> {
    let unit = AU_UNIT.lock();

    unit.as_ref()
        .unwrap()
        .next_snoops_reading()
        .map(|result| bincode::serialize::<SnoopsData>(&result).expect("serialize"))
}

#[wasm_bindgen]
pub fn unit_update(update: &[u8]) {
    let ev = bincode::deserialize::<UnitEV>(update).expect("deserialize");

    let mut unit = AU_UNIT.lock();
    let unit = unit.as_mut().expect("unit instance");

    unit.update(ev);
}
