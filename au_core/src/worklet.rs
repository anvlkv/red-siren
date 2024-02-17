use hecs::Entity;
use itertools::{interleave, Itertools};
use std::sync::{Arc, Mutex};
use wasm_bindgen::prelude::wasm_bindgen;

use crate::{Unit, UnitEV};

lazy_static::lazy_static! {
  static ref AU_UNIT: Arc<Mutex<Option<Unit>>> = Default::default();
}

thread_local! {
    static CALLBACK: Arc<Mutex<Box<dyn FnMut(&[f32]) -> [[f32; 128];2]>>> = Arc::new(Mutex::new(Box::new(|_| [[0_f32; 128];2])));
}

#[wasm_bindgen]
pub fn instantiate_unit() {
    CALLBACK.with(|callback| {
        let mut unit = Unit::new();
        unit.run(callback.as_ref()).expect("run unit");
        _ = AU_UNIT.lock().unwrap().insert(unit);
    })
}

#[wasm_bindgen]
pub fn process_samples(samples: &[f32]) -> Vec<f32> {
    CALLBACK.with(|callback| {
        let mut callback = callback.lock().unwrap();

        let [ch1, ch2] = callback(samples);

        interleave(ch1, ch2).collect()
    })
}

#[wasm_bindgen]
pub fn get_fft_data() -> Vec<u8> {
    let result = match AU_UNIT.try_lock() {
        Ok(unit) => {
            let unit = unit.as_ref().expect("unit instance");
            unit.next_fft_reading()
        }
        _ => None,
    };

    bincode::serialize::<Option<Vec<(f32, f32)>>>(&result).expect("serialize")
}

#[wasm_bindgen]
pub fn get_snoops_data() -> Vec<u8> {
    let result = match AU_UNIT.try_lock() {
        Ok(unit) => {
            let unit = unit.as_ref().expect("unit instance");
            unit.next_snoops_reading()
        }
        _ => None,
    };

    bincode::serialize::<Option<Vec<(Entity, Vec<f32>)>>>(&result).expect("serialize")
}

#[wasm_bindgen]
pub fn unit_update(update: &[u8]) {
    let ev = bincode::deserialize::<UnitEV>(update).expect("deserialize");

    let mut unit = AU_UNIT.lock().unwrap();
    let unit = unit.as_mut().expect("unit instance");

    unit.update(ev);
}
