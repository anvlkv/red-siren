use std::collections::HashMap;

use fundsp::hacker32::*;
use hecs::Entity;
use ringbuf::{Consumer, HeapConsumer, HeapProducer, HeapRb, Producer};

use crate::buf::AppAuBuffer;

use super::{node::Node, System};

const RENDER_BURSTS: usize = 5;

pub struct Backend {
    input_be: BigBlockAdapter32,
    output_be: BlockRateAdapter32,
    input_rb: HeapRb<f32>,
    analyze_exchange: Vec<f32>,
    nodes: HashMap<Entity, Node>,
    snoops: Vec<(Snoop<f32>, Entity)>,
    app_au_buffer: AppAuBuffer,
    sample_rate: u32,
}

impl Backend {
    pub fn new(
        sys: &mut System,
        snoops: Vec<(Snoop<f32>, Entity)>,
        fft_res: usize,
        app_au_buffer: AppAuBuffer,
    ) -> Self {
        let input_rb = HeapRb::new(fft_res * RENDER_BURSTS);
        let (input_be, output_be) = sys.backends();
        let nodes = sys.nodes.clone();
        let analyze_exchange = vec![0_f32; fft_res];
        Self {
            input_be,
            output_be,
            input_rb,
            nodes,
            app_au_buffer,
            analyze_exchange,
            snoops,
            sample_rate: sys.sample_rate,
        }
    }

    fn input_fn(
        data: &[f32],
        mut produce_analyze: Producer<f32, &HeapRb<f32>>,
        mut input_be: BigBlockAdapter32,
    ) {
        let mut processed_input = vec![0_f32; data.len()];
        input_be.process(data.len(), &[data], &mut [processed_input.as_mut_slice()]);

        let inserted = produce_analyze.push_iter(&mut processed_input.into_iter());

        if inserted != data.len() {
            log::warn!("input overflow: {}", data.len() - inserted)
        } else {
            log::trace!("store input: {}", inserted);
        }
    }

    fn analyze_fn(
        mut consume_analyze: Consumer<f32, &HeapRb<f32>>,
        analyze_exchange_slice: &mut [f32],
        nodes: &HashMap<Entity, Node>,
        sample_rate: u32,
        app_au_buffer: AppAuBuffer,
    ) {
        if consume_analyze.len() >= analyze_exchange_slice.len() {
            let written = consume_analyze.pop_slice(analyze_exchange_slice);
            debug_assert_eq!(written, analyze_exchange_slice.len());
            let fft = System::process_input_data(analyze_exchange_slice, nodes, sample_rate);
            app_au_buffer.push_fft_data(fft);
        }
    }

    fn output_fn(data: &mut [f32], mut output_be: BlockRateAdapter32, channels: usize) {
        for frame in data.chunks_mut(channels) {
            if channels == 1 {
                frame[0] = output_be.get_mono()
            } else {
                let sample = output_be.get_stereo();
                frame[0] = sample.0;
                frame[1] = sample.1;
            }
        }
    }

    pub fn produce_snoops_readig(
        mut snoops: Vec<(Snoop<f32>, Entity)>,
        app_au_buffer: AppAuBuffer,
    ) {
        let mut readings = Vec::new();
        for (snoop, _) in snoops.iter_mut() {
            if let Some(buf) = snoop.get() {
                let mut data = Vec::new();
                for i in 0..buf.len() {
                    data.push(buf.at(i))
                }
                readings.push(data)
            } else {
                if readings.len() > 0 {
                    log::warn!("incomplete snoops reading");
                    readings.clear();
                }
                break;
            }
        }

        if readings.len() > 0 {
            app_au_buffer.push_snoops_data(readings)
        }
    }

    pub fn make_split(
        &mut self,
        channels: u32,
    ) -> (Box<dyn FnMut(&[f32])>, Box<dyn FnMut(&mut [f32])>) {
        // let (mut produce_analyze, mut consume_analyze) = self.input_rb.split_ref();

        // let input = {
        //     let app_au_buffer = self.app_au_buffer.clone();
        //     let nodes = self.nodes.clone();
        //     let mut analyze_exchange = self.analyze_exchange;
        //     let sample_rate = self.sample_rate;
        //     let input_be = self.input_be.clone();

        //     move |data: &[f32]| {
        //         Self::input_fn(data, produce_analyze, input_be);
        //         Self::analyze_fn(
        //             consume_analyze,
        //             analyze_exchange.as_mut_slice(),
        //             &nodes,
        //             sample_rate,
        //             app_au_buffer,
        //         );
        //     }
        // };

        // let output = {
        //     let app_au_buffer = self.app_au_buffer.clone();
        //     let output_be = self.output_be;
        //     let snoops = self.snoops;
        //     move |data: &mut [f32]| {
        //         Self::output_fn(data, output_be, channels as usize);
        //         Self::produce_snoops_readig(snoops, app_au_buffer)
        //     }
        // };

        // (Box::new(input), Box::new(output))
        todo!()
    }
}
