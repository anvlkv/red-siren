use crate::components::Page;
use leptos::prelude::*;
use shared::RouteId;

#[component]
pub fn Donate() -> impl IntoView {
    view! {
        <Page route_id=RouteId::Donate title="Donate" route_back=RouteId::About>
            <div class="flex flex-col items-center justify-center gap-6">
                <h2 class="text-2xl italic">"Support the Creator of Red Siren"</h2>
                <p class="text-xl max-w-[42ch] ">
                    "Red Siren is a labor of love — designed, built, and shared with a passion for sound and creativity."
                </p>
                <p class="text-xl max-w-[42ch] ">
                    "If Red Siren has sparked something in you, consider donating. Every bit helps, and I'm deeply grateful for your kindness. Thank you for believing in this work."
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
                <p class="text-xl max-w-[42ch]">
                    "Your support helps me keep going. It fuels the time, care, and resources bringing this project to life."
                </p>
            </div>
        </Page>
    }
}
