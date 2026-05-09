mod audio_session;
mod engine;
mod mic_permission;
pub mod stream;

pub use audio_session::ensure_configured as ensure_audio_session_configured;
pub use engine::make_stream_controller;
pub use mic_permission::check_mic_permission;

// Re-export commonly used control types for ergonomics.
pub use stream::{Control, ControlInvocationResult};

pub mod prelude {
    pub use super::stream::{
        GenType, ProdType, STREAM_TIMEOUT_S, spawn_owned_input_stream, spawn_owned_output_stream,
    };
    pub use super::{
        Control, ControlInvocationResult, check_mic_permission, ensure_audio_session_configured,
        make_stream_controller,
    };
}
