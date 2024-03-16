use std::{
    process,
    sync::{mpsc::channel, Arc},
};

use app_core::{Effect, Event, RedSiren, RedSirenCapabilities, Request, ViewModel};
use crux_core::{
    capability::{Capability, CapabilityContext, Operation},
    Core,
};
use futures::{
    channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender},
    SinkExt, StreamExt,
};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct AppOp;

impl Operation for AppOp {
    type Output = ();
}

#[derive(Capability, Clone)]
pub struct AppCapability<Ev> {
    context: CapabilityContext<AppOp, Ev>,
    pub app: Arc<Core<Effect, RedSiren>>,
    ev_sender: Arc<Mutex<UnboundedSender<Event>>>,
    eff_receiver: Arc<Mutex<UnboundedReceiver<Effect>>>,
    resolve_eff_sender: Arc<Mutex<UnboundedSender<Effect>>>,
    view_receiver: Arc<Mutex<UnboundedReceiver<ViewModel>>>,
}

impl<Ev> AppCapability<Ev>
where
    Ev: 'static,
{
    pub fn new(context: CapabilityContext<AppOp, Ev>) -> Self {
        let (ev_sender, mut ev_receiver) = unbounded::<Event>();
        let (mut eff_sender, eff_receiver) = unbounded::<Effect>();
        let (resolve_eff_sender, mut resolve_eff_receiver) = unbounded::<Effect>();
        let (mut view_sender, view_receiver) = unbounded::<ViewModel>();
        let app = Arc::new(Core::new::<RedSirenCapabilities>());

        let process_effects = move |effects: Vec<Effect>| async {
            for effect in effects {
                match effect {
                    Effect::Render(_) => {
                        view_sender.send(app.view()).await;
                    }
                    _ => {
                        eff_sender.feed(effect).await;
                    }
                }
            }
            eff_sender.flush().await;
        };

        context.spawn({
            let app = app.clone();
            let view_sender = view_sender.clone();
            let eff_sender = eff_sender.clone();
            let process_effects = process_effects.clone();
            async move {
                while let Some(ev) = ev_receiver.next().await {
                    let effects = app.process_event(ev);
                    process_effects(effects).await;
                }
            }
        });

        context.spawn({
            let app = app.clone();
            let view_sender = view_sender.clone();
            let eff_sender = eff_sender.clone();
            let process_effects = process_effects.clone();
            async move {
                while let Some(effects) = resolve_eff_receiver.next().await {
                    process_effect(effects).await;
                }
            }
        });

        Self {
            context,
            app,
            ev_sender: Arc::new(Mutex::new(ev_sender)),
            eff_receiver: Arc::new(Mutex::new(eff_receiver)),
            view_receiver: Arc::new(Mutex::new(view_receiver)),
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

    pub fn receive_view<F>(&self, notify: F)
    where
        F: Fn(ViewModel) -> Ev + Send + 'static,
    {
        self.context.spawn({
            let context = self.context.clone();
            let recv = self.view_receiver.clone();

            async move {
                let mut recv = recv.lock();
                while let Some(view) = recv.next().await {
                    context.update_app(notify(view))
                }
            }
        })
    }

    pub fn forward_event(&self, ev: Event) {
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
            let app = self.app.clone();
            let rersolve_eff_sender = self.rersolve_eff_sender.clone();

            async move {
                let effects = app.resolve(request, result);
                let mut sender = rersolve_eff_sender.lock();
                sender.send_all(&mut effects.into_iter()).await;
            }
        })
    }
}
