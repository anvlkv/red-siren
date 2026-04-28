use std::sync::Mutex;

use common::{instrument::BandChannel, NodeKey};
use fundsp::prelude::*;

pub struct NodeHandle {
    pub channel: BandChannel,
    pub key: NodeKey,
    pub accentuation: Shared,
    pub rhythm: Shared,
    pub excitement_snoop_hs: Snoop,
    pub excitement_snoop_rad: Snoop,
    pub output_snoop: Snoop,
}

pub(super) struct InnerHandle {
    pub channel: BandChannel,
    pub key: NodeKey,
    pub accentuation: Shared,
    pub rhythm: Shared,
    excitement_snoop_hs: (Snoop, Mutex<Option<An<SnoopBackend>>>),
    excitement_snoop_rad: (Snoop, Mutex<Option<An<SnoopBackend>>>),
    output_snoop: (Snoop, Mutex<Option<An<SnoopBackend>>>),
}

impl InnerHandle {
    const SNOOP_CAPACITY_XCT: usize = 128;
    const SNOOP_CAPACITY_OUTPUT: usize = 1024;

    pub fn new(key: NodeKey, channel: BandChannel) -> Self {
        let excitement_snoop_hs = snoop(Self::SNOOP_CAPACITY_XCT);
        let excitement_snoop_rad = snoop(Self::SNOOP_CAPACITY_XCT);
        let output_snoop = snoop(Self::SNOOP_CAPACITY_OUTPUT);
        Self {
            channel,
            key,
            accentuation: shared(0.0),
            rhythm: shared(0.0),
            excitement_snoop_hs: (
                excitement_snoop_hs.0,
                Mutex::new(Some(excitement_snoop_hs.1)),
            ),
            excitement_snoop_rad: (
                excitement_snoop_rad.0,
                Mutex::new(Some(excitement_snoop_rad.1)),
            ),
            output_snoop: (output_snoop.0, Mutex::new(Some(output_snoop.1))),
        }
    }

    pub fn into_outer(self) -> NodeHandle {
        let InnerHandle {
            channel,
            key,
            accentuation,
            rhythm,
            excitement_snoop_hs,
            excitement_snoop_rad,
            output_snoop,
        } = self;

        NodeHandle {
            channel,
            key,
            accentuation,
            rhythm,
            excitement_snoop_hs: excitement_snoop_hs.0,
            excitement_snoop_rad: excitement_snoop_rad.0,
            output_snoop: output_snoop.0,
        }
    }

    #[must_use]
    pub fn take_excitement_snoop_hs(&self) -> An<SnoopBackend> {
        self.excitement_snoop_hs.1.lock().unwrap().take().unwrap()
    }

    #[must_use]
    pub fn take_excitement_snoop_rad(&self) -> An<SnoopBackend> {
        self.excitement_snoop_rad.1.lock().unwrap().take().unwrap()
    }

    #[must_use]
    pub fn take_output_snoop(&self) -> An<SnoopBackend> {
        self.output_snoop.1.lock().unwrap().take().unwrap()
    }
}
