//! Feedback pass and catch nodes for creating feedback loops in the audio graph.

use std::sync::Arc;

use fundsp::{prelude::*, thingbuf::ThingBuf};

const FEEDBACK_PASS_ID: u64 = crate::util::hash_str(concat!(module_path!(), "::FeedbackPass"));
const FEEDBACK_CATCH_ID: u64 = crate::util::hash_str(concat!(module_path!(), "::FeedbackCatch"));
const FEEDBACK_PASS_BUFFER_SIZE: usize = 128;

#[derive(Clone)]
/// A node that takes an input signal and stores it in a buffer for later retrieval by a corresponding FeedbackCatch node.
/// The output of this node is the same as the input signal, allowing it to be used in a feedback loop without altering the signal.
pub struct FeedbackPass(Arc<ThingBuf<f32>>);

impl AudioNode for FeedbackPass {
    const ID: u64 = FEEDBACK_PASS_ID;

    type Inputs = U1;
    type Outputs = U1;

    fn tick(&mut self, input: &Frame<f32, Self::Inputs>) -> Frame<f32, Self::Outputs> {
        let value = input[0];
        match self.0.push(value) {
            Ok(_) => {}
            Err(v) => {
                _ = self.0.pop();
                _ = self.0.push(v.into_inner()).ok();
            }
        }
        Frame::from([value])
    }
}

#[derive(Clone)]
/// A node that retrieves the most recent value stored by a corresponding FeedbackPass node. If the buffer is empty, it outputs 0.0. This allows it to be used in a feedback loop to access the previous output of the loop.
pub struct FeedbackCatch(Arc<ThingBuf<f32>>);

impl AudioNode for FeedbackCatch {
    const ID: u64 = FEEDBACK_CATCH_ID;

    type Inputs = U0;
    type Outputs = U1;

    fn tick(&mut self, _input: &Frame<f32, Self::Inputs>) -> Frame<f32, Self::Outputs> {
        let value = self.0.pop().unwrap_or(0.0);
        Frame::from([value])
    }
}

/// Creates a pair of FeedbackPass and FeedbackCatch nodes that share a buffer for passing values between them. The FeedbackPass node will store incoming values in the buffer, while the FeedbackCatch node will retrieve the most recent value from the buffer when ticked.
pub fn create_feedback_pass() -> (An<FeedbackPass>, An<FeedbackCatch>) {
    let buffer = Arc::new(ThingBuf::new(FEEDBACK_PASS_BUFFER_SIZE));
    (An(FeedbackPass(buffer.clone())), An(FeedbackCatch(buffer)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use fundsp::prelude32::sine_hz;
    use insta_fun::prelude::*;

    fn snapshot_config() -> SnapshotConfig {
        SnapshotConfigBuilder::default()
            .sample_rate(44100.0)
            .num_samples(512)
            .build()
            .unwrap()
    }

    #[test]
    fn feedback_pass_catch_with_sine_input() {
        let (pass, catch) = create_feedback_pass();

        let freq = 440.0;
        assert_audio_unit_snapshot!(
            "feedback_pass_catch_sine_loop",
            sine_hz(freq) >> pass | catch,
            InputSource::None,
            snapshot_config()
        );
    }
}
