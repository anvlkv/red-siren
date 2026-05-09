use std::sync::Arc;

use common::{instrument::BandChannel, NodeKey};
use fundsp::prelude::*;
use parking_lot::Mutex;

use crate::feedback_pass::{create_feedback_pass, FeedbackCatch, FeedbackPass};
use crate::system::excitor::control::Control;

pub struct NodeHandle {
    pub channel: BandChannel,
    pub key: NodeKey,
    pub accentuation: Shared,
    pub rhythm: Shared,
    pub excitement_snoop_hs: Arc<Mutex<Snoop>>,
    pub excitement_snoop_rad: Arc<Mutex<Snoop>>,
    pub output_snoop: Arc<Mutex<Snoop>>,
    pub control: Control,
}

pub(super) struct InnerHandle {
    pub channel: BandChannel,
    pub key: NodeKey,
    pub accentuation: Shared,
    pub rhythm: Shared,
    pub control: Control,
    feedback: (
        Mutex<Option<An<FeedbackCatch>>>,
        Mutex<Option<An<FeedbackPass>>>,
    ),
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
        let fb_pass = create_feedback_pass();
        let control = Control {
            key,
            real: shared(0.0),
            imaginary: shared(0.0),
        };
        Self {
            channel,
            key,
            control,
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
            feedback: (Mutex::new(Some(fb_pass.1)), Mutex::new(Some(fb_pass.0))),
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
            feedback,
            control,
        } = self;

        assert!(
            feedback.0.lock().is_none() && feedback.1.lock().is_none(),
            "Feedback nodes must be taken before converting to NodeHandle"
        );
        assert!(
            excitement_snoop_hs.1.lock().is_none(),
            "Excitement snoop HS must be taken before converting to NodeHandle"
        );
        assert!(
            excitement_snoop_rad.1.lock().is_none(),
            "Excitement snoop RAD must be taken before converting to NodeHandle"
        );
        assert!(
            output_snoop.1.lock().is_none(),
            "Output snoop must be taken before converting to NodeHandle"
        );

        NodeHandle {
            channel,
            control,
            key,
            accentuation,
            rhythm,
            excitement_snoop_hs: Arc::new(Mutex::new(excitement_snoop_hs.0)),
            excitement_snoop_rad: Arc::new(Mutex::new(excitement_snoop_rad.0)),
            output_snoop: Arc::new(Mutex::new(output_snoop.0)),
        }
    }

    #[must_use]
    pub fn take_excitement_snoop_hs(&self) -> An<SnoopBackend> {
        self.excitement_snoop_hs
            .1
            .lock()
            .take()
            .expect("already taken")
    }

    #[must_use]
    pub fn take_excitement_snoop_rad(&self) -> An<SnoopBackend> {
        self.excitement_snoop_rad
            .1
            .lock()
            .take()
            .expect("already taken")
    }

    #[must_use]
    pub fn take_output_snoop(&self) -> An<SnoopBackend> {
        self.output_snoop.1.lock().take().expect("already taken")
    }

    #[must_use]
    pub fn take_feedback_pass(&self) -> An<FeedbackPass> {
        self.feedback.1.lock().take().expect("already taken")
    }

    #[must_use]
    pub fn take_feedback_catch(&self) -> An<FeedbackCatch> {
        self.feedback.0.lock().take().expect("already taken")
    }
}
