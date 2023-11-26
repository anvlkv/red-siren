use std::{collections::HashMap, rc::Rc, cell::RefCell};
use leptos::*;

#[derive(Clone)]
pub struct KVContext(Rc<RefCell<HashMap<String, Vec<u8>>>>);

impl KVContext {
    pub fn provide() {
        provide_context(Self(Rc::new(RefCell::new(HashMap::new()))));
    }

    pub fn get(&self) -> Rc<RefCell<HashMap<String, Vec<u8>>>> {
        self.0.clone()
    }
}
