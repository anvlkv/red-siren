use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Serialize, Deserialize, Debug)]
pub struct Band {
    pub channel: super::BandChannel,
    pub medium: crate::body::materials::Medium,
}
