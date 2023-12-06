use crux_core::capability::{CapabilityContext, Operation};
use crux_macros::Capability;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum NavigateOperation {
    To(super::Activity),
}

impl Operation for NavigateOperation {
    type Output = ();
}

#[derive(Capability)]
pub struct Navigate<Ev> {
    context: CapabilityContext<NavigateOperation, Ev>,
}

impl<Ev> Navigate<Ev>
where
    Ev: 'static,
{
    pub fn new(context: CapabilityContext<NavigateOperation, Ev>) -> Self {
        Self { context }
    }

    pub fn to(&self, activity: super::Activity) {
        let ctx = self.context.clone();
        self.context.spawn(async move {
            let _ = ctx
                .request_from_shell(NavigateOperation::To(activity))
                .await;
        });
    }
}
