use crate::components::{Button, ContentPage, Icon, UiVariant};
use leptos::prelude::*;
use shared::{commands::health::MicPermissionPayload, RouteId};
use tauri_use::{use_command, use_invoke, UseTauriReturn, UseTauriWithReturn};

#[component]
pub fn Permissions() -> impl IntoView {
    let (prompted, set_prompted) = signal(false);

    let UseTauriReturn {
        error: mic_permission_error,
        trigger: mic_permission_trigger,
        data: mic_permission_data,
    } = use_invoke::<MicPermissionPayload, (), bool>(
        shared::commands::health::GRANT_MIC_PREMISSION,
    );

    Effect::new(move |_| {
        if let Some(err) = mic_permission_error() {
            log::error!(
                "Error invoking {}: {}",
                shared::commands::health::GRANT_MIC_PREMISSION,
                err
            );

            set_prompted(false);
        }
    });

    let UseTauriWithReturn {
        error: nav_resume_error,
        trigger: nav_resume_trigger,
        ..
    } = use_command::<()>(shared::commands::navigation::NAV_RESUME);

    Effect::new(move |_| {
        if let Some(err) = nav_resume_error() {
            log::error!(
                "Error invoking {}: {}",
                shared::commands::navigation::NAV_RESUME,
                err
            );
        }
    });

    Effect::new(move |_| {
        if mic_permission_data().is_some() {
            nav_resume_trigger(Some(()));
        }
    });

    view! {
        <ContentPage route_id=RouteId::Permissions title="Permissions">
            <div class="flex flex-col items-center justify-center gap-6">
                <h2 class="text-2xl text-bold max-w-[42ch] italic">
                    "Why grant microphone access"
                </h2>
                <p class="text-xl max-w-[42ch] ">
                    "Granting mic access lets Red Siren respond to your noise in real time. Audio is processed locally — nothing is recorded or sent off‑device. It can run without mic access, but tuning and responsiveness will be reduced"
                </p>
                <h3 class="text-2xl text-bold max-w-[42ch] italic">"How to enable it"</h3>
                <p class="text-xl max-w-[42ch] ">
                    "When the system prompt appears, choose Allow. If you previously denied access, re-enable it in System Settings → Privacy & Security → Microphone for the app, then restart the app and try again."
                </p>
                <div class="grid grid-cols-2 justify-items-stretch gap-4 w-full">
                    <Button
                        on:click=move |_| {
                            mic_permission_trigger(
                                Some((
                                    MicPermissionPayload {
                                        prompt: true,
                                    },
                                    (),
                                )),
                            );
                            set_prompted(true)
                        }
                        class="relative pl-14"
                        attr:aria-label="Allow mic access"
                        disabled=prompted
                    >
                        <span class="absolute left-4 text-4xl">
                            <Icon name="mic" />
                        </span>
                        "Allow"
                    </Button>
                    <Button
                        variant=UiVariant::Outline
                        on:click=move |_| {
                            mic_permission_trigger(
                                Some((
                                    MicPermissionPayload {
                                        prompt: false,
                                    },
                                    (),
                                )),
                            );
                            set_prompted(true)
                        }
                        attr:aria-label="Skip mic access"
                        disabled=prompted
                    >
                        "Skip"
                        <strong class="ml-2 opacity-75">"(Limited)"</strong>
                    </Button>
                </div>
            </div>
        </ContentPage>
    }
}
