#![cfg_attr(feature = "ssr", allow(unused_variables, unused_imports, dead_code))]

use cfg_if::cfg_if;
use leptos::ev::change;
use leptos::*;
use leptos_use::{use_event_listener, use_window};
use std::cell::RefCell;
use std::rc::Rc;

type RemoveListener = Rc<RefCell<Option<Box<dyn Fn()>>>>;

/// as seen in [leptos_use::use_media_query] implementation
pub fn use_dppx() -> Signal<f64> {
    let (dppx, set_dppx) = create_signal(1.0 as f64);
    let window = use_window();

    cfg_if! { if #[cfg(feature = "hydrate")] {
      let media_query: Rc<RefCell<Option<web_sys::MediaQueryList>>> = Rc::new(RefCell::new(None));
      let remove_listener: RemoveListener = Rc::new(RefCell::new(None));

      let cleanup = {
          let remove_listener = Rc::clone(&remove_listener);

          move || {
              if let Some(remove_listener) = remove_listener.take().as_ref() {
                  remove_listener();
              }
          }
      };

      let listener = Rc::new(RefCell::new(Rc::new(|_| {}) as Rc<dyn Fn(web_sys::Event)>));

      let update = {
          let cleanup = cleanup.clone();
          let listener = Rc::clone(&listener);

          Rc::new(move || {
              cleanup();
              let window = window.as_ref().unwrap();
              let dppx = window.device_pixel_ratio();
              let query = format!("(resolution: {}dppx)", dppx);

              let mut media_query = media_query.borrow_mut();
              *media_query = window.match_media(&query).unwrap_or(None);

              let listener = Rc::clone(&*listener.borrow());

              remove_listener.replace(Some(Box::new(use_event_listener(
                  media_query.clone(),
                  change,
                  move |e| listener(e),
              ))));

              set_dppx(dppx);
          })
      };

      {
          let update = Rc::clone(&update);
          listener.replace(Rc::new(move |_| update()) as Rc<dyn Fn(web_sys::Event)>);
      }

      create_effect(move |_| { update() });

      on_cleanup(cleanup);
    }}

    dppx.into()
}
