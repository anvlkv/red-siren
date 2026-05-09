mod composition;
pub mod consts;
mod static_path;
mod wavering;

use common::RouteId;
use composition::IntroComp;
use leptos::prelude::*;
use leptos_router::{hooks::use_location, location::Location};

#[component]
pub fn Intro(children: ChildrenFn) -> impl IntoView {
    let Location { pathname, .. } = use_location();
    let is_intro_fading = Memo::new(move |_| {
        pathname()
            .parse::<RouteId>()
            .map(|route| !route.is_content())
            .unwrap_or(true)
    });

    view! {
        <div class="contents">
            <IntroComp exit=is_intro_fading />
            {move || children()}
        </div>
    }
}
