use leptos::prelude::*;
use leptos_use::use_timeout_fn;
use leptos_use::UseTimeoutFnReturn;

use super::consts::*;
use super::static_path::*;
use super::wavering::*;

#[component]
pub fn IntroComp(#[prop(into)] exit: Signal<bool>) -> impl IntoView {
    let (paused, set_paused) = signal(false);
    let (fade_out, set_fadeout) = signal(false);

    let UseTimeoutFnReturn {
        start: start_exit_timeout,
        stop: stop_exit_timeout,
        ..
    } = use_timeout_fn(
        move |_: ()| {
            set_paused(true);
        },
        INTRO_FADE_DURATION_MS,
    );

    Effect::new(move || {
        if exit() {
            start_exit_timeout(());
            set_fadeout(true);
        } else {
            set_paused(false);
            set_fadeout(false);
            stop_exit_timeout();
        }
    });

    let animation_style = Signal::derive(move || {
        format!(
            "animation: {} {}ms forwards;",
            if fade_out() { "fadeOut" } else { "fadeIn" },
            INTRO_FADE_DURATION_MS
        )
    });

    view! {
        <div
            class="absolute h-screen w-screen splash-picture overflow-hidden"
            role="img"
            style=animation_style
        >
            <svg
                viewBox="0 0 1048 932"
                fill="none"
                class="absolute h-full w-auto top-0 left-0 stroke-gray dark:stroke-cinnabar blur-[.5px]"
                r#xmlns="http://www.w3.org/2000/svg"
                id="waves"
            >
                <Wavering paused />
            </svg>
            <svg
                viewBox="0 0 430 932"
                class="absolute h-full w-auto top-auto left-auto bottom-0 right-0 fill-red dark:fill-black stroke-black dark:stroke-red"
                xmlns="http://www.w3.org/2000/svg"
                id="stone"
            >
                <path d=STONE_PATH stroke-width="3" />
            </svg>
            <svg
                viewBox="0 0 430 932"
                class="absolute h-full w-auto top-auto left-auto bottom-0 right-0 fill-black dark:fill-red"
                xmlns="http://www.w3.org/2000/svg"
                id="siren"
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
                id="flute-shadow"
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
                id="flute"
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
                id="sun"
            >
                <circle r=INTRO_SUN_RADIUS cx=INTRO_SUN_POS_X cy=INTRO_SUN_POS_Y />
            </svg>
            <svg
                viewBox="0 0 430 932"
                class="absolute h-full w-auto top-auto left-auto bottom-0 right-0 fill-black dark:fill-red stroke-red dark:stroke-black"
                xmlns="http://www.w3.org/2000/svg"
                id="siren-arm"
            >
                <path d=SIREN_ARM_PATH stroke-width="2" />
            </svg>
            <svg
                viewBox="0 0 430 932"
                class="absolute h-full w-auto top-auto left-auto right-0 bottom-0 fill-black dark:fill-red"
                xmlns="http://www.w3.org/2000/svg"
                id="siren-front"
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
