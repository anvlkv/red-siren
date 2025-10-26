use leptos::prelude::*;
use leptos_router::{hooks::use_query, params::Params};

#[derive(PartialEq, Eq, Params, Clone, Copy)]
struct SecondaryWindow {
    secondary: Option<bool>,
}

pub fn is_secondary_window() -> Memo<bool> {
    let query = use_query::<SecondaryWindow>();

    Memo::new(move |_| query().is_ok_and(|SecondaryWindow { secondary }| secondary == Some(true)))
}
