pub mod adsr_gate;
pub mod bpm_grid;
pub mod encoder;
pub mod scheduler;
pub mod signal;

use std::collections::HashMap;

use common::{config::NodeKey, error::{InstrumentError, Result}};
use num_rational::Rational32;
pub(self) use signal::FrameEncodedSignal;

use fundsp::{prelude::*, typenum::Unsigned};
use u_num_it::u_num_it;

use crate::system::{
    adsr_3d::create_adsr_3d,
    grid::{
        adsr_gate::create_adsr_gate,
        bpm_grid::{create_bpm_grid, BpmGrid},
        encoder::{create_scheduling_request_encoder, SchedulingRequestEncoderHandle},
        scheduler::create_scheduler,
    },
    node::{Node, NumNodeInputs},
};

#[derive(Clone)]
pub struct Grid {
    metro_net: Net,
    bpm: Shared,
    schedulers: HashMap<NodeKey, SchedulingRequestEncoderHandle>,
}

impl Grid {
    pub fn new<F: Real + 'static>(initial_bpm: F, keys: &[NodeKey]) -> Self {
        let mut schedulers = HashMap::new();
        let mut metro_net = Net::new(0, keys.len() * NumNodeInputs::USIZE);
        let bpm = shared(convert(initial_bpm));
        let bpm_grid_id = metro_net.push(Box::new(var(&bpm) >> create_bpm_grid::<F>(initial_bpm)));

        let split_id = u_num_it!(
            1..71,
            match keys.len() {
                U => {
                    metro_net.push(Box::new(multisplit::<
                        <BpmGrid<F> as AudioNode>::Outputs,
                        NumType,
                    >()))
                }
            }
        );

        metro_net.pipe_all(bpm_grid_id, split_id);

        for (i, key) in keys.iter().enumerate() {
            let (handle, encoder) = create_scheduling_request_encoder();
            schedulers.insert(*key, handle);
            let gate_id = metro_net.push(Box::new(
                (multipass::<<BpmGrid<F> as AudioNode>::Outputs>() | encoder)
                    >> create_scheduler::<F>()
                    >> create_adsr_gate::<NumNodeInputs>()
                    >> create_adsr_3d::<F, NumNodeInputs>(),
            ));
            let num_grid_outputs = <BpmGrid<F> as AudioNode>::Outputs::USIZE;
            for j in 0..num_grid_outputs {
                metro_net.connect(split_id, j + i * num_grid_outputs, gate_id, j);
            }

            metro_net.pipe_output(gate_id);
        }

        metro_net.check();

        Self {
            metro_net,
            bpm,
            schedulers,
        }
    }

    pub fn backend(&mut self) -> NetBackend {
        self.metro_net.backend()
    }

    pub async fn schedule_event(&self, key: NodeKey, time: Rational32, duration: Rational32, repeat: Option<u32>, event: f64) -> Result<()> {
        if let Some(handle) = self.schedulers.get(&key) {
            handle
                .send(time, duration, repeat, event)
                .await
        } else {
            Err(InstrumentError::UnknownNodeKey(key).into())
        }
    }

    pub fn set_bpm<F: Real>(&mut self, new_bpm: F) {
        self.bpm.set(convert(new_bpm));
    }
}
