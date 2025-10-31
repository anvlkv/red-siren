use crate::components::{ContentPage, UiPlacement};
use leptos::prelude::*;

#[component]
pub fn Donate() -> impl IntoView {
    view! {
        <ContentPage title="Donate" card_animation_direction=UiPlacement::Left>
            <div class="flex flex-col items-center justify-center gap-6">
                <h2 class="md:text-2xl text-lg italic">"Support the Creator of Red Siren"</h2>
                <p class="md:text-xl text-base max-w-md lg:max-w-[42ch] ">
                    "Red Siren is my practice in listening and making. I craft it with care and share it freely, so it can meet your place and time."
                </p>
                <p class="md:text-xl text-base max-w-md lg:max-w-[42ch] ">
                    "If it offered focus, curiosity, or calm, your donation helps me continue—turning hours, tools, and attention into sound."
                </p>

                <div class="contents">
                    <a
                        href="https://nowpayments.io/donation?api_key=5014fba8-64de-4526-84c1-527cd621d274"
                        target="_blank"
                        rel="noreferrer noopener"
                    >
                        <img
                            src="https://nowpayments.io/images/embeds/donation-button-black.svg"
                            alt="Crypto donation button by NOWPayments"
                        />
                    </a>
                </div>
                <p class="md:text-xl text-base max-w-md lg:max-w-[42ch]">
                    "Thank you. I’m a.nvlkv — your support lets me listen longer and keep tending Red Siren."
                </p>
            </div>
        </ContentPage>
    }
}
