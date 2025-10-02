use leptos::prelude::*;

use super::static_path::*;
use super::wavering::*;

#[component]
pub fn IntroComp() -> impl IntoView {
    view! {
        <div
            class="absolute h-screen w-screen splash-picture overflow-hidden opacity-(--intro-opacity)"
            role="img"
        >
            <svg
                viewBox="0 0 1048 932"
                fill="none"
                class="absolute h-full w-auto top-0 left-0 stroke-gray dark:stroke-cinnabar blur-[.5px]"
                r#xmlns="http://www.w3.org/2000/svg"
            >
                <Wavering />
            </svg>
            <svg
                viewBox="0 0 430 932"
                class="absolute h-full w-auto top-auto left-auto bottom-0 right-0 fill-red dark:fill-black stroke-black dark:stroke-red"
                xmlns="http://www.w3.org/2000/svg"
            >
                <path d=STONE_PATH stroke-width="3" />
            </svg>
            <svg
                viewBox="0 0 430 932"
                class="absolute h-full w-auto top-auto left-auto bottom-0 right-0 fill-black dark:fill-red"
                xmlns="http://www.w3.org/2000/svg"
            >
                <path d=SIREN_PATH_1 />
                <path d=SIREN_PATH_2 />
                <path d=SIREN_PATH_3 />
            </svg>
            <svg
                viewBox="0 0 430 932"
                fill="none"
                class="absolute h-full w-auto top-auto left-auto bottom-0 right-0 stroke-red dark:stroke-black"
                xmlns="http://www.w3.org/2000/svg"
            >
                <rect
                    x="73.7113"
                    y="576.054"
                    width="53.653"
                    height="8.25253"
                    transform="rotate(-17.1246 48.3365 585.964)"
                    stroke-width="2"
                />
            </svg>
            <svg
                viewBox="0 0 430 932"
                class="absolute h-full w-auto top-auto left-auto bottom-0 right-0 fill-red dark:fill-black stroke-black dark:stroke-red"
                xmlns="http://www.w3.org/2000/svg"
            >
                <rect
                    width="282.096"
                    height="4.25253"
                    x="48.3365"
                    y="585.964"
                    transform="rotate(-17.1246 48.3365, 585.964)"
                    stroke-width="2"
                />
            </svg>
            <svg
                viewBox="0 0 430 932"
                class="absolute h-full w-auto top-0 left-0 fill-black dark:fill-red"
                xmlns="http://www.w3.org/2000/svg"
            >
                <circle r="39" cx="107" cy="164" />
            </svg>
            <svg
                viewBox="0 0 430 932"
                class="absolute h-full w-auto top-auto left-auto bottom-0 right-0 fill-black dark:fill-red stroke-red dark:stroke-black"
                xmlns="http://www.w3.org/2000/svg"
            >
                <path d=SIREN_ARM_PATH stroke-width="2" />
            </svg>
            <svg
                viewBox="0 0 430 932"
                class="absolute h-full w-auto top-auto left-auto right-0 bottom-0 fill-black dark:fill-red"
                xmlns="http://www.w3.org/2000/svg"
            >
                <path d=SIREN_FRONT_PATH_1 />
                <path d=SIREN_FRONT_PATH_2 />
                <path d=SIREN_FRONT_PATH_3 />
                <path d=SIREN_FRONT_PATH_4 />
                <path d=SIREN_FRONT_PATH_5 />
            </svg>
        </div>
    }
}
