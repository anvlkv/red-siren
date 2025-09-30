use leptos::prelude::*;
use shared::{commands::navigation::NavigateRequestPayload, RouteId};

use tauri_use::{use_invoke_with_args, UseTauriWithReturn};

use crate::components::{Card, CardAnimation, NavigationTx};

use super::item::{MenuItem, MenuItemView};

/// Edge placement for the compact menu bar.
#[derive(Clone, Copy, Default)]
#[allow(dead_code)]
pub enum CompactMenuPlacement {
    #[default]
    Bottom,
    Top,
    Left,
    Right,
}

const LEAVE_MS: f64 = 400.0;
const APPEAR_MS: f64 = 650.0;

#[component]
pub fn CompactMenu(
    #[prop(into)] items: Signal<Vec<MenuItem>>,
    #[prop(optional)] placement: CompactMenuPlacement,
    children: Children,
) -> impl IntoView {
    let nav_tx = expect_context::<Signal<Option<NavigationTx>>>();
    // Materialize children once; avoids re-calling a possibly FnMut closure in reactive nodes.
    let child_view = children();

    let UseTauriWithReturn {
        trigger: trigger_navigate,
        error,
        ..
    } = use_invoke_with_args::<NavigateRequestPayload, ()>(shared::commands::navigation::NAVIGATE);

    // Trigger to notify backend that the enter animation has completed
    let UseTauriWithReturn {
        trigger: enter_done_trigger,
        error: enter_error,
        ..
    } = use_invoke_with_args::<shared::commands::navigation::NavTxPayload, ()>(
        shared::commands::navigation::NAV_ENTER_DONE,
    );

    // Trigger to notify backend that the leave animation has completed
    let UseTauriWithReturn {
        trigger: leave_done_trigger,
        error: leave_error,
        ..
    } = use_invoke_with_args::<shared::commands::navigation::NavTxPayload, ()>(
        shared::commands::navigation::NAV_LEAVE_DONE,
    );

    // Animation state
    let (menu_anim, set_menu_anim) = signal(None::<CardAnimation>);
    let (docked, set_docked) = signal(false);

    // Initial appear animation (replays each mount)
    Effect::new(move |_| {
        if menu_anim().is_some() || docked() {
            return;
        }
        set_menu_anim(Some(CardAnimation::Appear {
            x_from_px: 0.0,
            y_from_px: -200.0,      // slide downward to final dock position
            tilt_x_from_deg: -75.0, // strong forward flip
            ms: APPEAR_MS,
        }));
    });

    // Queue leave animation on navigation leave
    Effect::new(move |_| {
        if matches!(nav_tx(), Some(NavigationTx::Leave(_))) {
            let leave = match placement {
                CompactMenuPlacement::Bottom => CardAnimation::LeaveTiltX {
                    from_deg: 0.0,
                    to_deg: 90.0,
                    ms: LEAVE_MS,
                },
                CompactMenuPlacement::Top => CardAnimation::LeaveTiltX {
                    from_deg: 0.0,
                    to_deg: -90.0,
                    ms: LEAVE_MS,
                },
                CompactMenuPlacement::Left => CardAnimation::LeaveY {
                    from_deg: 0.0,
                    to_deg: -90.0,
                    ms: LEAVE_MS,
                },
                CompactMenuPlacement::Right => CardAnimation::LeaveY {
                    from_deg: 0.0,
                    to_deg: 90.0,
                    ms: LEAVE_MS,
                },
            };
            set_menu_anim(Some(leave));
        }
    });

    // Animation completion callback
    let on_anim_done = Callback::new(move |_| {
        match nav_tx() {
            Some(NavigationTx::Enter(tx_id)) => {
                enter_done_trigger(Some(shared::commands::navigation::NavTxPayload { tx_id }));
            }
            Some(NavigationTx::Leave(tx_id)) => {
                leave_done_trigger(Some(shared::commands::navigation::NavTxPayload { tx_id }));
            }
            None => {}
        }

        set_menu_anim(None);
        if !docked.get_untracked() {
            set_docked(true);
        }
    });

    Effect::new(move |_| {
        if let Some(err) = error() {
            log::error!(
                "Error invoking {}: {}",
                shared::commands::navigation::NAVIGATE,
                err
            );
        }

        if let Some(err) = enter_error() {
            log::error!(
                "Error invoking {}: {}",
                shared::commands::navigation::NAV_ENTER_DONE,
                err
            );
        }

        if let Some(err) = leave_error() {
            log::error!(
                "Error invoking {}: {}",
                shared::commands::navigation::NAV_LEAVE_DONE,
                err
            );
        }
    });

    // Edge positioning container (self-contained so outer wrapper on page not needed)
    let edge_container_cls = match placement {
        CompactMenuPlacement::Bottom => {
            "fixed inset-x-0 bottom-0 flex justify-center pointer-events-none"
        }
        CompactMenuPlacement::Top => {
            "fixed inset-x-0 top-0 flex justify-center pointer-events-none"
        }
        CompactMenuPlacement::Left => {
            "fixed inset-y-0 left-0 flex items-center pointer-events-none"
        }
        CompactMenuPlacement::Right => {
            "fixed inset-y-0 right-0 flex items-center pointer-events-none"
        }
    };

    let card_variant = Signal::derive(move || {
        if !docked() {
            "".to_string()
        } else {
            match placement {
                CompactMenuPlacement::Bottom => "rounded-b-none",
                CompactMenuPlacement::Top => "rounded-t-none",
                CompactMenuPlacement::Left => "rounded-l-none",
                CompactMenuPlacement::Right => "rounded-r-none",
            }
            .to_string()
        }
    });

    view! {
        <div class=edge_container_cls>
            <Card
                padding="Md".to_string()
                class=card_variant
                start_animation=Signal::derive(menu_anim)
                on_animation_done=on_anim_done
            >
                <div class="flex items-center gap-4 pointer-events-auto">
                    <button on:click=move |_| {
                        trigger_navigate(
                            Some(NavigateRequestPayload {
                                route: RouteId::Home,
                            }),
                        );
                    }>
                        <h1 class="block text-3xl italic cursor-pointer hover:underline focus:underline">
                            "Red Siren"
                        </h1>
                    </button>
                    {child_view}
                    {move || {
                        items()
                            .iter()
                            .copied()
                            .map(|item| {
                                view! { <MenuItemView item trigger_navigate compact=true /> }
                            })
                            .collect_view()
                    }}
                </div>
            </Card>
        </div>
    }
}
