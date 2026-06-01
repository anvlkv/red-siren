use fundsp::prelude::*;

const MEMO_ID: u64 = crate::util::hash_str(concat!(module_path!(), "::Memo"));

#[derive(Clone)]
pub struct Memo<X: AudioNode> {
    inner: X,
    tick_last_input: Option<Frame<f32, X::Inputs>>,
    tick_last_output: Option<Frame<f32, X::Outputs>>,
    process_last_size: usize,
    process_has_cache: bool,
    process_last_input: Vec<f32>,
    process_last_output: Vec<f32>,
}

impl<X: AudioNode> Memo<X> {
    pub fn new(inner: X) -> Self {
        Self {
            inner,
            tick_last_input: None,
            tick_last_output: None,
            process_last_size: 0,
            process_has_cache: false,
            process_last_input: Vec::new(),
            process_last_output: Vec::new(),
        }
    }

    pub fn inner(&self) -> &X {
        &self.inner
    }

    pub fn inner_mut(&mut self) -> &mut X {
        &mut self.inner
    }

    fn clear_cache(&mut self) {
        self.tick_last_input = None;
        self.tick_last_output = None;
        self.process_has_cache = false;
        self.process_last_size = 0;
    }

    fn process_input_index(channel: usize, frame: usize, size: usize) -> usize {
        channel * size + frame
    }

    fn process_output_index(channel: usize, frame: usize, size: usize) -> usize {
        channel * size + frame
    }
}

impl<X: AudioNode> AudioNode for Memo<X>
where
    X::Inputs: Size<f32>,
    X::Outputs: Size<f32>,
{
    const ID: u64 = MEMO_ID;

    type Inputs = X::Inputs;
    type Outputs = X::Outputs;

    fn reset(&mut self) {
        self.inner.reset();
        self.clear_cache();
    }

    fn set_sample_rate(&mut self, sample_rate: f64) {
        self.inner.set_sample_rate(sample_rate);
        self.reset();
    }

    fn tick(&mut self, input: &Frame<f32, Self::Inputs>) -> Frame<f32, Self::Outputs> {
        if let (Some(last_input), Some(last_output)) = (
            self.tick_last_input.as_ref(),
            self.tick_last_output.as_ref(),
        ) {
            if last_input == input {
                return Frame::generate(|channel| last_output[channel]);
            } else {
                log::debug!(
                    "Cache miss in tick for Memo node, input: {:?}, last_input: {:?}",
                    input,
                    last_input
                );
            }
        } else {
            log::debug!("No cache in tick for Memo node, input: {:?}", input,);
        }

        let output = self.inner.tick(input);
        self.tick_last_input = Some(Frame::generate(|channel| input[channel]));
        self.tick_last_output = Some(Frame::generate(|channel| output[channel]));
        output
    }

    fn process(&mut self, size: usize, input: &BufferRef, output: &mut BufferMut) {
        debug_assert!(input.channels() == self.inputs());
        debug_assert!(output.channels() == self.outputs());

        if self.process_has_cache && self.process_last_size == size {
            let mut matches = true;

            'compare: for channel in 0..self.inputs() {
                for frame in 0..size {
                    let cached =
                        self.process_last_input[Self::process_input_index(channel, frame, size)];
                    if input.at_f32(channel, frame) != cached {
                        matches = false;
                        break 'compare;
                    }
                }
            }

            if matches {
                for channel in 0..self.outputs() {
                    for frame in 0..size {
                        let cached = self.process_last_output
                            [Self::process_output_index(channel, frame, size)];
                        output.set_f32(channel, frame, cached);
                    }
                }
                return;
            }
        }

        self.inner.process(size, input, output);

        let input_len = self.inputs() * size;
        let output_len = self.outputs() * size;
        if self.process_last_input.len() != input_len {
            self.process_last_input.resize(input_len, 0.0);
        }
        if self.process_last_output.len() != output_len {
            self.process_last_output.resize(output_len, 0.0);
        }

        for channel in 0..self.inputs() {
            for frame in 0..size {
                let index = Self::process_input_index(channel, frame, size);
                self.process_last_input[index] = input.at_f32(channel, frame);
            }
        }
        for channel in 0..self.outputs() {
            for frame in 0..size {
                let index = Self::process_output_index(channel, frame, size);
                self.process_last_output[index] = output.at_f32(channel, frame);
            }
        }
        self.process_last_size = size;
        self.process_has_cache = true;
    }

    fn set(&mut self, setting: Setting) {
        self.inner.set(setting);
    }

    fn ping(&mut self, probe: bool, hash: AttoHash) -> AttoHash {
        self.inner.ping(probe, hash.hash(Self::ID))
    }

    fn allocate(&mut self) {
        self.inner.allocate();
    }

    fn route(&mut self, input: &SignalFrame, frequency: f64) -> SignalFrame {
        self.inner.route(input, frequency)
    }
}

pub fn memo<X: AudioNode>(inner: X) -> An<Memo<X>>
where
    X::Inputs: Size<f32>,
    X::Outputs: Size<f32>,
{
    An(Memo::new(inner))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    struct CountingNode {
        tick_calls: usize,
        process_calls: usize,
    }

    impl CountingNode {
        fn new() -> Self {
            Self {
                tick_calls: 0,
                process_calls: 0,
            }
        }
    }

    impl AudioNode for CountingNode {
        const ID: u64 = 0xC011EC7;
        type Inputs = U1;
        type Outputs = U1;

        fn tick(&mut self, input: &Frame<f32, Self::Inputs>) -> Frame<f32, Self::Outputs> {
            self.tick_calls += 1;
            Frame::from([input[0] + self.tick_calls as f32])
        }

        fn process(&mut self, size: usize, input: &BufferRef, output: &mut BufferMut) {
            self.process_calls += 1;
            let k = self.process_calls as f32;
            for i in 0..size {
                output.set_f32(0, i, input.at_f32(0, i) + k);
            }
        }
    }

    #[test]
    fn memo_tick_reuses_when_input_unchanged() {
        let mut node = Memo::new(CountingNode::new());

        let x = Frame::from([1.0]);
        let y1 = node.tick(&x);
        let y2 = node.tick(&x);

        assert_eq!(y1, y2);
        assert_eq!(node.inner().tick_calls, 1);

        let z = Frame::from([2.0]);
        let _ = node.tick(&z);
        assert_eq!(node.inner().tick_calls, 2);
    }

    #[test]
    fn memo_process_reuses_whole_block_when_unchanged() {
        let mut node = Memo::new(CountingNode::new());

        let mut input = BufferArray::<U1>::new();
        let mut output1 = BufferArray::<U1>::new();
        let mut output2 = BufferArray::<U1>::new();
        let size = 8;

        for i in 0..size {
            input.set_f32(0, i, i as f32 * 0.5);
        }

        node.process(size, &input.buffer_ref(), &mut output1.buffer_mut());
        node.process(size, &input.buffer_ref(), &mut output2.buffer_mut());

        for i in 0..size {
            assert_eq!(output1.at_f32(0, i), output2.at_f32(0, i));
        }
        assert_eq!(node.inner().process_calls, 1);

        input.set_f32(0, 3, 42.0);
        node.process(size, &input.buffer_ref(), &mut output2.buffer_mut());
        assert_eq!(node.inner().process_calls, 2);
    }

    #[test]
    fn memo_clears_cache_on_reset_and_sample_rate_change() {
        let mut node = Memo::new(CountingNode::new());
        let x = Frame::from([3.0]);

        let _ = node.tick(&x);
        let _ = node.tick(&x);
        assert_eq!(node.inner().tick_calls, 1);

        node.reset();
        let _ = node.tick(&x);
        assert_eq!(node.inner().tick_calls, 2);

        let _ = node.tick(&x);
        assert_eq!(node.inner().tick_calls, 2);

        node.set_sample_rate(48_000.0);
        let _ = node.tick(&x);
        assert_eq!(node.inner().tick_calls, 3);
    }
}
