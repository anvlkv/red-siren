use leptos::prelude::*;
use leptos_router::{hooks::use_query, params::Params};

use super::boot_flags::boot_flags;

#[derive(PartialEq, Eq, Params, Clone, Copy)]
struct SecondaryWindow {
    secondary: Option<bool>,
}

pub fn is_secondary_window() -> Memo<bool> {
    let default_secondary = boot_flags().secondary;
    let query = use_query::<SecondaryWindow>();

    Memo::new(move |_| {
        default_secondary
            || query().is_ok_and(|SecondaryWindow { secondary }| secondary == Some(true))
    })
}
