pub mod input;
pub mod output;
pub mod random;

use std::sync::mpsc::Sender;

pub use input::*;
pub use output::*;
pub use random::*;

pub const STREAM_TIMEOUT_S: u64 = 10;

/// Result of a control invocation, sent back to the caller per request.
pub type ControlInvocationResult = Result<(), String>;

/// Control messages for the stream owner.
pub enum Control {
    Pause(Sender<ControlInvocationResult>),
    Resume(Sender<ControlInvocationResult>),
    Shutdown(Sender<ControlInvocationResult>),
}
