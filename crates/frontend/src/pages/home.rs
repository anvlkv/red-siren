use leptos::prelude::*;

#[component]
pub fn Home() -> impl IntoView {
    view! {
        <section class="reset-home flex items-center justify-center px-6 py-12">
            <div class="mx-auto flex max-w-3xl flex-col gap-5 text-center">
                <p class="text-xs uppercase tracking-[0.45em] sm:text-sm">"Architecture reset"</p>
                <h1 class="text-5xl leading-none sm:text-7xl">"Red Siren"</h1>
                <p class="text-base leading-7 sm:text-lg">
                    "Phase 1 keeps the app runnable while the previous instrument, tuner, and intro flows are cleared out."
                </p>
            </div>
        </section>
    }
}
