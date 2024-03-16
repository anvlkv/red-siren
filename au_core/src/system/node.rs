use ::shared::NodeData;
use fundsp::hacker32::*;
use hecs::Entity;
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct Node {
    pub f_base: Shared<f32>,
    pub f_emit: (Shared<f32>, Shared<f32>),
    pub f_sense: ((Shared<f32>, Shared<f32>), (Shared<f32>, Shared<f32>)),
    pub control: Shared<f32>,
    pub button: Entity,
    pub pan: Shared<f32>,
}

impl std::fmt::Debug for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let d: NodeData = self.into();
        write!(f, "{d:?}")
    }
}

impl From<NodeData> for Node {
    fn from(value: NodeData) -> Self {
        Self {
            f_base: shared(value.f_base),
            f_emit: (shared(value.f_emit.0), shared(value.f_emit.1)),
            f_sense: (
                (shared(value.f_sense.0 .0), shared(value.f_sense.0 .1)),
                (shared(value.f_sense.1 .0), shared(value.f_sense.1 .1)),
            ),
            control: shared(value.control),
            button: value.button,
            pan: shared(value.pan),
        }
    }
}

impl Into<NodeData> for &Node {
    fn into(self) -> NodeData {
        NodeData {
            f_base: self.f_base.value(),
            f_emit: (self.f_emit.0.value(), self.f_emit.1.value()),
            f_sense: (
                (self.f_sense.0 .0.value(), self.f_sense.0 .1.value()),
                (self.f_sense.1 .0.value(), self.f_sense.1 .1.value()),
            ),
            control: self.control.value(),
            button: self.button,
            pan: self.pan.value(),
        }
    }
}

impl Serialize for Node {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let data: NodeData = self.into();
        data.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Node {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let data: NodeData = NodeData::deserialize(deserializer)?;
        Ok(data.into())
    }
}

impl Node {
    pub fn new(button: Entity) -> Node {
        Node {
            button,
            f_base: shared(0.0),
            f_emit: (shared(0.0), shared(0.0)),
            f_sense: ((shared(0.0), shared(0.0)), (shared(0.0), shared(0.0))),
            control: shared(0.0),
            pan: shared(0.0),
        }
    }

    pub fn dummy() -> Self {
        Self::new(Entity::DANGLING)
    }
}
