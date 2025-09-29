use tokio::sync::Mutex;

#[derive(Default, Debug, Clone, Copy)]
pub enum ActivationSource {
    #[default]
    Entropy,
    Mic,
}

impl From<u8> for ActivationSource {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::Entropy,
            _ => Self::Mic,
        }
    }
}

impl Into<u8> for ActivationSource {
    fn into(self) -> u8 {
        match self {
            Self::Entropy => 0,
            Self::Mic => 1,
        }
    }
}

#[derive(Default)]
pub struct InstrumentEngine {
    pub(super) inner: Inner,
}

#[derive(Default)]
pub(super) struct Inner {
    pub playing: Mutex<bool>,
    pub activation_source: Mutex<ActivationSource>,
}
