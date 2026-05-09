use crate::components::{Button, ContentPage, Icon, UiPlacement, UiSize};
use common::RouteId;
use leptos::prelude::*;

#[component]
pub fn About() -> impl IntoView {
    view! {
        <ContentPage title="About" card_animation_direction=UiPlacement::Left>
            <div class="flex flex-col items-center justify-center gap-6">
                <p class="md:text-xl text-base max-w-md lg:max-w-[42ch]">
                    <strong class="md:text-2xl text-lg italic">
                        "Red Siren is a noise chime:"
                    </strong>
                    <span>
                        " a call and an answer, a mirror of breath, traffic, kettle, wind. It sings only when you do, a vessel of tone. It does not repeat the world; it refracts it into sound. You will not hear the source; you will hear its answer."
                    </span>
                </p>
                <p class="md:text-xl text-base max-w-md lg:max-w-[42ch]">
                    "Begin as you are. Let chance or the room speak first, or guide it with your hands. Touch the keys; hold several at once. Each touch bends the whole. Slide along them to shift the grain and glow. Place a few quiet listeners where you want it to hear. Tilt the mood toward bright or toward dark."
                </p>
                <p class="md:text-xl text-base max-w-md lg:max-w-[42ch] ">
                    "Sometimes you will know what called it; sometimes not. Play until the edges soften. When you stop, notice what remains."
                </p>
                <p class="md:text-xl text-base max-w-md lg:max-w-[42ch] ">
                    "Red Siren is free and open source under the CC‑BY‑SA license — take the code, remix it, and share what you make. If it speaks to you, show some love:"
                    <a
                        class="font-bold ml-2 underline"
                        href="https://github.com/anvlkv/red-siren"
                        target="_blank"
                        aria-label="Star anvlkv/red-siren on GitHub"
                    >
                        "Star me on GitHub"
                    </a>
                </p>
                <Button
                    href=RouteId::Donate.as_ref()
                    attr:aria-label="Donations"
                    size=UiSize::Lg
                    class="w-full"
                >
                    <Icon name="donate" size=UiSize::Lg />
                    <span class="inline-block flex-grow text-center">"Help me"</span>
                </Button>
            </div>
        </ContentPage>
    }
}
