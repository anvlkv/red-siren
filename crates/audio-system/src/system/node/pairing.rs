use common::NodeKey;
use fundsp::typenum::Unsigned;
use fundsp::{net::NodeId, prelude::AudioNode};

use crate::grid::RhythmGrid;

use super::controller::NodeController;

pub(super) struct ExcitementPairing {
    pub net: NodeId,
    pub band_index: usize,
    pub in_band_node_index: usize,
    pub prior_channel_nodes: usize,
    pub prior_global_nodes: usize,
    pub key: NodeKey,
}

impl ExcitementPairing {
    const CTRL_INPUTS_LEN: usize = <NodeController<f32> as AudioNode>::Inputs::USIZE; // 2;
    const RHYTHM_DATA_LEN: usize = <RhythmGrid<f32> as AudioNode>::Outputs::USIZE; // 3;

    pub fn source_hit(&self) -> usize {
        self.global_node_index() * Self::CTRL_INPUTS_LEN
    }

    pub fn source_radius(&self) -> usize {
        self.source_hit() + 1
    }

    pub fn target_hit(&self) -> usize {
        let local_j = self.prior_channel_nodes + self.in_band_node_index;
        Self::RHYTHM_DATA_LEN + local_j * Self::CTRL_INPUTS_LEN
    }

    pub fn target_radius(&self) -> usize {
        self.target_hit() + 1
    }

    fn global_node_index(&self) -> usize {
        self.prior_global_nodes + self.in_band_node_index
    }
}
