use crux_core::capability::Operation;
use derive_builder::Builder;
use hecs::{Bundle, Entity};
use serde::{Deserialize, Serialize};

pub type FFTData = Vec<(f32, f32)>;
pub type SnoopsData = Vec<Vec<f32>>;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum PlatformKind {
    Web,
    MobileWeb,
    MobileIos,
    MobileAndroid,
}

impl From<String> for PlatformKind {
    fn from(value: String) -> Self {
        match value.to_lowercase().as_str() {
            "web" => Self::Web,
            "mobile-web" => Self::MobileWeb,
            "ios" => Self::MobileIos,
            "android" => Self::MobileAndroid,
            _ => panic!("unknown platform"),
        }
    }
}

#[derive(Bundle, Deserialize, Serialize, Builder, Debug, Clone, Copy)]
pub struct NodeData {
    #[builder(default = "hecs::Entity::DANGLING")]
    pub button: Entity,
    pub f_base: f32,
    pub f_emit: (f32, f32),
    pub f_sense: ((f32, f32), (f32, f32)),
    #[builder(default = "0_f32")]
    pub control: f32,
    pub pan: f32,
}

#[cfg(target_arch = "wasm32")]
pub fn map_js_err(err: wasm_bindgen::JsValue) -> anyhow::Error {
    let description = format!("{:?}", err);
    anyhow::anyhow!("js err: {description}")
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq, Copy, Default)]
pub enum UnitState {
    #[default]
    None,
    Init,
    Playing,
    Paused,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub enum UnitResolve {
    Create,
    RunUnit(bool),
    UpdateEV(bool),
    FftData(FFTData),
    SnoopsData(SnoopsData),
}

impl Eq for UnitResolve {}

impl Operation for UnitResolve {
    type Output = ();
}
