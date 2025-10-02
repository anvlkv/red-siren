use leptos::{html, prelude::*};
use leptos_use::{use_element_size, UseElementSizeReturn};
use shared::{
    commands::navigation::NavigateRequestPayload, events::setup::SafeAreaInstestUiIncrementPayload,
    RouteId,
};

use tauri_use::use_invoke_with_args;

use crate::components::{Card, CardAnimation, NavigationTx, StretchAxis};

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
    #[prop(into)] placement: Signal<CompactMenuPlacement>,
    children: Children,
) -> impl IntoView {
    let nav_tx = expect_context::<Signal<Option<NavigationTx>>>();
    // Materialize children once; avoids re-calling a possibly FnMut closure in reactive nodes.
    let child_view = children();

    let el = NodeRef::<html::Div>::new();

    let UseElementSizeReturn {
        width: menu_width,
        height: menu_height,
    } = use_element_size(el);

    let tauri_use::UseTauriWithReturn {
        trigger: trigger_inset_update,
        error: inset_error,
        ..
    } = use_invoke_with_args::<shared::commands::setup::SafeAreaInstestUiIncrementPayload, ()>(
        shared::commands::setup::UI_SAFE_AREA_INSETS_APPLY,
    );

    let tauri_use::UseTauriWithReturn {
        trigger: trigger_navigate,
        error,
        ..
    } = use_invoke_with_args::<NavigateRequestPayload, ()>(shared::commands::navigation::NAVIGATE);

    // Trigger to notify backend that the enter animation has completed
    let tauri_use::UseTauriWithReturn {
        trigger: enter_done_trigger,
        error: enter_error,
        ..
    } = use_invoke_with_args::<shared::commands::navigation::NavTxPayload, ()>(
        shared::commands::navigation::NAV_ENTER_DONE,
    );

    // Trigger to notify backend that the leave animation has completed
    let tauri_use::UseTauriWithReturn {
        trigger: leave_done_trigger,
        error: leave_error,
        ..
    } = use_invoke_with_args::<shared::commands::navigation::NavTxPayload, ()>(
        shared::commands::navigation::NAV_LEAVE_DONE,
    );

    // Update safe area insets when menu height changes
    Effect::new(move || {
        let menu_height = menu_height() as f32;
        let menu_width = menu_width() as f32;
        let placement = placement();
        trigger_inset_update(Some(match placement {
            CompactMenuPlacement::Bottom => SafeAreaInstestUiIncrementPayload {
                top: 0_f32,
                right: 0_f32,
                bottom: menu_height,
                left: 0_f32,
            },
            CompactMenuPlacement::Top => SafeAreaInstestUiIncrementPayload {
                top: menu_height,
                right: 0_f32,
                bottom: 0_f32,
                left: 0_f32,
            },
            CompactMenuPlacement::Left => SafeAreaInstestUiIncrementPayload {
                top: 0_f32,
                right: 0_f32,
                bottom: 0_f32,
                left: menu_width,
            },
            CompactMenuPlacement::Right => SafeAreaInstestUiIncrementPayload {
                top: 0_f32,
                right: menu_width,
                bottom: 0_f32,
                left: 0_f32,
            },
        }))
    });

    // Animation state
    let (menu_anim, set_menu_anim) = signal(None::<CardAnimation>);
    let (docked, set_docked) = signal(false);

    // Initial appear animation (replays each mount)
    Effect::new(move |_| {
        if menu_anim().is_some() || docked() {
            return;
        }
        // Determine stretch axis based on current placement (horizontal bar stretches Y, vertical bar stretches X)
        let place = placement();
        let stretch_axis = match place {
            CompactMenuPlacement::Left | CompactMenuPlacement::Right => StretchAxis::X,
            _ => StretchAxis::Y,
        };
        // Depth & translation tuning (temporary placeholder values for new 3D system)
        let (from_x, from_y, from_z, tilt) = match place {
            CompactMenuPlacement::Bottom => (0.0, 140.0, -400.0, -25.0),
            CompactMenuPlacement::Top => (0.0, -140.0, -400.0, 25.0),
            CompactMenuPlacement::Left => (-140.0, 0.0, -400.0, -10.0),
            CompactMenuPlacement::Right => (140.0, 0.0, -400.0, -10.0),
        };
        set_menu_anim(Some(CardAnimation::Appear3D {
            from_x_px: from_x,
            from_y_px: from_y,
            from_z_px: from_z,
            from_tilt_x_deg: tilt,
            stretch_axis,
            stretch_factor: 1.08,
        }));
    });

    // Queue leave animation on navigation leave
    Effect::new(move |_| {
        let nav = nav_tx();
        let place = placement();
        if matches!(nav, Some(NavigationTx::Leave(_))) {
            // Travel out in direction consistent with placement (edge retract) with depth + yaw
            let (to_x, to_z, to_rot_y) = match place {
                CompactMenuPlacement::Bottom => (260.0_f32, -380.0_f32, -95.0_f32),
                CompactMenuPlacement::Top => (-260.0_f32, -380.0_f32, 95.0_f32),
                CompactMenuPlacement::Left => (-260.0_f32, -380.0_f32, 95.0_f32),
                CompactMenuPlacement::Right => (260.0_f32, -380.0_f32, -95.0_f32),
            };
            set_menu_anim(Some(CardAnimation::LeaveTravel3D {
                to_x_px: to_x,
                to_z_px: to_z,
                to_rot_y_deg: to_rot_y,
            }));
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

        if let Some(err) = inset_error() {
            log::error!(
                "Error invoking {}: {}",
                shared::commands::setup::UI_SAFE_AREA_INSETS_APPLY,
                err
            );
        }
    });

    // Edge positioning container (reactive to placement changes)
    let edge_container_cls = Signal::derive(move || match placement() {
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
    });

    let card_variant = Signal::derive(move || {
        let docked = docked();
        let placement = placement();
        if !docked {
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
        <div class=edge_container_cls node_ref=el>
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
