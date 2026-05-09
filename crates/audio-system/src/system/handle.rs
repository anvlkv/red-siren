use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use common::instrument::Config as InstrumentConfig;
use common::tuner::Config as TunerConfig;
use common::NodeKey;
use fundsp::shared::Shared;
use parking_lot::RwLock;

use crate::excitor::handle::ExcitorHandle;
use crate::node::NodeHandle;

pub struct SystemHandle {
    pub node_handles: Arc<BTreeMap<NodeKey, NodeHandle>>,
    pub excitor_handle: Arc<ExcitorHandle>,
    pub input_ny_thr: Arc<Shared>,
    pub input_ny_wd: Arc<Shared>,
}
