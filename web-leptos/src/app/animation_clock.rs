use futures::channel::mpsc::Sender;
#[allow(unused)]
use std::{
    rc::Rc,
    sync::{Arc, Mutex},
};

cfg_if::cfg_if! {
  if #[cfg(feature="ssr")] {
    pub struct AnimationClock;
  }
  else {
    pub struct AnimationClock {
      _callback: Arc<Mutex<Box<dyn Fn(f64)>>>,
      schedule_fn: Arc<Mutex<Box<dyn Fn()>>>,
      next_tick: Arc<Mutex<Option<i32>>>,
      senders: Arc<Mutex<Vec<Sender<f64>>>>,
    }
  }
}

impl AnimationClock {
    #[cfg(feature = "ssr")]
    pub fn new() -> Rc<Self> {
        Rc::new(Self)
    }

    #[cfg(not(feature = "ssr"))]
    pub fn new() -> Rc<Self> {
        use wasm_bindgen::{closure::Closure, JsCast};
        use web_sys::window;

        let senders = Arc::new(Mutex::new(vec![]));
        let next_tick = Arc::new(Mutex::new(None));

        let callback = Arc::new(Mutex::new(Box::new(|_: f64| {}) as Box<dyn Fn(f64)>));

        let schedule_fn = {
            let callback = Arc::clone(&callback);
            let next_tick = Arc::clone(&next_tick);

            move || {
                let win = window().unwrap();
                let callback = Arc::clone(&callback);
                let mut mtx = next_tick.lock().unwrap();
                *mtx = win
                    .request_animation_frame(
                        Closure::once_into_js(move |ts: f64| {
                            let cb = callback.lock().unwrap();
                            cb(ts);
                        })
                        .as_ref()
                        .unchecked_ref(),
                    )
                    .ok();
            }
        };

        let schedule_fn = Arc::new(Mutex::new(Box::new(schedule_fn) as Box<dyn Fn()>));

        let loop_fn = {
            let schedule_fn = Arc::clone(&schedule_fn);
            let senders = Arc::clone(&senders);

            move |ts: f64| {
                let mut senders = senders.lock().unwrap();
                match senders.iter_mut().try_for_each(|sender: &mut Sender<f64>| {
                    sender.try_send(ts)
                }) {
                  Ok(_) => {
                    let schedule = schedule_fn.lock().unwrap();
                    schedule();
                    log::trace!("animation tick");
                  }
                  Err(e) => {
                    log::error!("send timer tick error: {e:?}");
                  }
                }

            }
        };

        {
            let mut cb_mtx = callback.lock().unwrap();
            *cb_mtx = Box::new(loop_fn);
        }

        Rc::new(Self {
            _callback: callback,
            schedule_fn,
            next_tick,
            senders,
        })
    }

    #[allow(unused)]
    pub fn add_sender(&self, sender: Sender<f64>) {
        #[cfg(not(feature = "ssr"))]
        {
            {
                let mut mtx = match self.senders.lock() {
                    Ok(mtx) => mtx,
                    Err(e) => {
                        log::error!("senders poison: {e}");
                        e.into_inner()
                    }
                };

                mtx.push(sender);

                log::info!("add animation sender");
            }

            let should_schedule = match self.next_tick.lock() {
                Ok(mtx) => mtx,
                Err(e) => {
                    log::error!("next tick poison: {e}");
                    e.into_inner()
                }
            }
            .is_none();

            if should_schedule {
                let schedule = self.schedule_fn.lock().unwrap();
                schedule();

                log::info!("schedule animation");
            }
        }
    }

    pub fn clear(&self) {
        #[cfg(not(feature = "ssr"))]
        {
            {
                let mut mtx = match self.senders.lock() {
                    Ok(mtx) => mtx,
                    Err(e) => {
                        log::error!("senders poison: {e}");
                        e.into_inner()
                    }
                };

                mtx.clear();

                log::info!("clear animations");
            }

            let mut next = match self.next_tick.lock() {
                Ok(mtx) => mtx,
                Err(e) => {
                    log::error!("next tick poison: {e}");
                    e.into_inner()
                }
            };

            if let Some(next) = next.as_ref() {
                use web_sys::window;

                let win = window().unwrap();

                win.cancel_animation_frame(*next).unwrap();
            }

            *next = None;
        }
    }
}
