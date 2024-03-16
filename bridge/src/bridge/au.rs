use std::sync::Arc;

use au_core::{AUCapabilities, AUViewModel, Effect, RedSirenAU, UnitEvent};
use crux_core::{
    capability::{Capability, CapabilityContext, Operation},
    Core,
};
use futures::channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct AUOp;

impl Operation for AUOp {
    type Output = ();
}

#[derive(Capability, Clone)]
pub struct AUCapability<Ev> {
    context: CapabilityContext<AUOp, Ev>,
    au: Arc<Core<Effect, RedSirenAU>>,
    ev_sender: Arc<Mutex<UnboundedSender<UnitEvent>>>,
    eff_receiver: Arc<Mutex<UnboundedReceiver<Effect>>>,
    resolve_eff_sender: Arc<Mutex<UnboundedSender<Effect>>>,
}

impl<Ev> AUCapability<Ev>
where
    Ev: 'static,
{
    pub fn new(context: CapabilityContext<AppOp, Ev>) -> Self {
        let (ev_sender, mut ev_receiver) = unbounded::<UnitEvent>();
        let (mut eff_sender, eff_receiver) = unbounded::<Effect>();
        let (resolve_eff_sender, mut resolve_eff_receiver) = unbounded::<Effect>();

        let au = Arc::new(Core::new::<AUCapabilities>());

        context.spawn({
            let eff_sender = eff_sender.clone();
            async move {
                while let Some(ev) = ev_receiver.next().await {
                    let effects = au.process_event(ev);
                    eff_sender.send_all(&mut effects.into_iter()).await;
                }
            }
        });

        context.spawn({
            let eff_sender = eff_sender.clone();
            async move {
                while let Some(effect) = resolve_eff_receiver.next().await {
                    eff_sender.send(effect).await;
                }
            }
        });

        Self {
            context,
            au,
            ev_sender: Arc::new(Mutex::new(ev_sender)),
            eff_receiver: Arc::new(Mutex::new(eff_receiver)),
            resolve_eff_sender: Arc::new(Mutex::new(resolve_eff_sender)),
        }
    }

    pub fn receive_effects<F>(&self, notify: F)
    where
        F: Fn(Effect) -> Ev + Send + 'static,
    {
        self.context.spawn({
            let context = self.context.clone();
            let recv = self.eff_receiver.clone();

            async move {
                let mut recv = recv.lock();
                while let Some(effect) = recv.next().await {
                    context.update_app(notify(effect))
                }
            }
        })
    }

    pub fn forward_event(&self, ev: UnitEvent) {
        self.context.spawn({
            let context = self.context.clone();
            let sender = self.ev_sender.clone();
            async move {
                let mut sender = sender.lock();
                sender.send(ev).await;
            }
        })
    }

    pub fn resolve<Op>(&self, request: &mut Request<Op>, result: Op)
    where
        Op: Operation,
    {
        self.context.spawn({
            let au = self.au.clone();
            let rersolve_eff_sender = self.rersolve_eff_sender.clone();

            async move {
                let effects = au.resolve(request, result);
                let mut sender = rersolve_eff_sender.lock();
                sender.send_all(&mut effects.into_iter()).await;
            }
        })
    }
}
