use std::sync::mpsc::Sender;

/// Maximum seconds CPAL waits inside build/playback callbacks before timing out.
pub const STREAM_TIMEOUT_S: u64 = 10;

pub const CONTROL_INVOKE_TIMEOUT_MS: u64 = 500;

/// Result of a control invocation, sent back to the caller per request.
pub type ControlInvocationResult = Result<(), String>;

/// Control messages for the stream owner thread.
pub enum Control {
    /// Pause stream processing (acknowledged via provided sender).
    Pause(Sender<ControlInvocationResult>),
    /// Resume stream processing (acknowledged via provided sender).
    Resume(Sender<ControlInvocationResult>),
    /// Terminate the stream / worker. Receiver should drop resources,
    /// then acknowledge and exit its loop.
    Shutdown(Sender<ControlInvocationResult>),
}
