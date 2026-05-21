mod audio_session;
mod stream;

pub mod engine;
pub mod gate_manager;
pub mod rt_subsystem;
pub mod telemetry;

#[cfg(test)]
pub mod test_host;

#[cfg(test)]
mod tests;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProcessingMode {
    None,
    #[default]
    Tick,
    Process,
    ProcessBig,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum ExcitementSource {
    #[default]
    Entropy,
    Mic,
}

impl From<u8> for ExcitementSource {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::Entropy,
            _ => Self::Mic,
        }
    }
}

impl From<ExcitementSource> for u8 {
    fn from(value: ExcitementSource) -> Self {
        match value {
            ExcitementSource::Entropy => 0,
            ExcitementSource::Mic => 1,
        }
    }
}
