use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Band {
    pub channel: super::BandChannel,
    pub medium: crate::body::materials::Medium,
}
