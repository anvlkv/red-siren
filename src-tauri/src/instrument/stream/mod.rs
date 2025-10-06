mod input;
mod output;
mod random;

use std::sync::mpsc::Sender;

pub use input::*;
pub use output::*;
pub use random::*;

/// Result of a control invocation, sent back to the caller per request.
pub type ControlInvocationResult = Result<(), String>;

/// Control messages for the stream owner.
pub enum Control {
    Pause(Sender<ControlInvocationResult>),
    Resume(Sender<ControlInvocationResult>),
    Shutdown(Sender<ControlInvocationResult>),
}
