use derive_builder::Builder;
use hecs::Entity;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Builder, Debug)]
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

pub type FFTData = Vec<(f32, f32)>;
pub type SnoopsData = Vec<Vec<f32>>;

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub enum UnitResolve {
    RunUnit(bool),
    UpdateEV(bool),
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq, Copy, Default)]
pub enum UnitState {
    #[default]
    None,
    Playing,
    Paused,
}

#[derive(Deserialize, Serialize, Debug)]
pub enum UnitEV {
    ButtonPressed(Entity),
    ButtonReleased(Entity),
    Detune(Entity, f32),
    Configure(Vec<NodeData>),
    SetControl(Entity, f32),
    ListenToInput(bool),
    Suspend,
    Resume,
}
