use leptos::{html, prelude::*};
use leptos_use::{use_element_size, use_window_size, UseElementSizeReturn};
use shared::{
    commands::navigation::NavigateRequestPayload, events::setup::SafeAreaInstestUiIncrementPayload,
    RouteId,
};

use tauri_use::use_invoke_with_args;

use crate::components::{Card, CardAnimation, EdgeSide, NavigationTx, UiPlacement};

use super::item::{MenuItem, MenuItemView};

#[component]
pub fn CompactMenu(
    #[prop(into)] items: Signal<Vec<MenuItem>>,
    #[prop(into)] placement: Signal<UiPlacement>,
    children: Children,
) -> impl IntoView {
    let nav_tx = expect_context::<Signal<Option<NavigationTx>>>();
    // Materialize children once
    let child_view = children();

    let el = NodeRef::<html::Div>::new();
    // Track window size (reactive) for dynamic, non-hardcoded animation distances
    let window_size = use_window_size();

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

    let tauri_use::UseTauriWithReturn {
        trigger: enter_done_trigger,
        error: enter_error,
        ..
    } = use_invoke_with_args::<shared::commands::navigation::NavTxPayload, ()>(
        shared::commands::navigation::NAV_ENTER_DONE,
    );

    let tauri_use::UseTauriWithReturn {
        trigger: leave_done_trigger,
        error: leave_error,
        ..
    } = use_invoke_with_args::<shared::commands::navigation::NavTxPayload, ()>(
        shared::commands::navigation::NAV_LEAVE_DONE,
    );

    // Track previous inset values to prevent unnecessary updates and loops
    let prev_insets = RwSignal::new(None::<SafeAreaInstestUiIncrementPayload>);

    // Update safe area insets when menu size or placement changes
    Effect::new(move |_| {
        let menu_height = menu_height() as f32;
        let menu_width = menu_width() as f32;
        let placement = placement();

        // Skip if dimensions are invalid
        if menu_height <= 0.0 && menu_width <= 0.0 {
            return;
        }

        let new_insets = match placement {
            UiPlacement::Bottom => SafeAreaInstestUiIncrementPayload {
                top: 0.0,
                right: 0.0,
                bottom: menu_height,
                left: 0.0,
            },
            UiPlacement::Top => SafeAreaInstestUiIncrementPayload {
                top: menu_height,
                right: 0.0,
                bottom: 0.0,
                left: 0.0,
            },
            UiPlacement::Left => SafeAreaInstestUiIncrementPayload {
                top: 0.0,
                right: 0.0,
                bottom: 0.0,
                left: menu_width,
            },
            UiPlacement::Right => SafeAreaInstestUiIncrementPayload {
                top: 0.0,
                right: menu_width,
                bottom: 0.0,
                left: 0.0,
            },
        };

        // Only trigger if values have changed significantly (>1px threshold)
        let should_update = match prev_insets() {
            None => true,
            Some(prev) => {
                (new_insets.top - prev.top).abs() > 1.0
                    || (new_insets.right - prev.right).abs() > 1.0
                    || (new_insets.bottom - prev.bottom).abs() > 1.0
                    || (new_insets.left - prev.left).abs() > 1.0
            }
        };

        if should_update {
            prev_insets.set(Some(new_insets));
            trigger_inset_update(Some(new_insets));
        }
    });

    // Derived signals for layout-dependent values to prevent remounts
    let side = Signal::derive(move || match placement() {
        UiPlacement::Top => EdgeSide::Top,
        UiPlacement::Bottom => EdgeSide::Bottom,
        UiPlacement::Left => EdgeSide::Left,
        UiPlacement::Right => EdgeSide::Right,
    });

    let is_vertical = Signal::derive(move || placement().is_vertical());

    // Animation parameters derived from placement and window size
    let enter_anim_params = Signal::derive(move || {
        let w = window_size.width.get() as f32;
        let h = window_size.height.get() as f32;
        match side() {
            EdgeSide::Top | EdgeSide::Bottom => (h * 0.18, -w * 0.35, 0.0),
            EdgeSide::Left => (w * 0.14, 0.0, -w * 0.15),
            EdgeSide::Right => (w * 0.14, 0.0, -w * 0.15),
        }
    });

    let leave_anim_params = Signal::derive(move || {
        let w = window_size.width.get() as f32;
        let h = window_size.height.get() as f32;
        match side() {
            EdgeSide::Top | EdgeSide::Bottom => (h * 0.22, -w * 0.32, 0.0),
            EdgeSide::Left => (w * 0.18, 0.0, -w * 0.2),
            EdgeSide::Right => (w * 0.18, 0.0, -w * 0.2),
        }
    });

    // Animation state
    let (docked, set_docked) = signal(false);
    let last_leave_tx_id = RwSignal::new(None::<u64>);

    let menu_anim = Memo::new(move |_| {
        let docked = docked();
        let current_side = side();
        let (offset_px_enter, depth_z_px_enter, yaw_deg_enter) = enter_anim_params();
        let (offset_px_leave, depth_z_px_leave, yaw_deg_leave) = leave_anim_params();

        let nav_tx = nav_tx();

        if let Some(NavigationTx::Leave(_)) = nav_tx {
            //leave
            Some(CardAnimation::EdgeLeave3D {
                side: current_side,
                offset_px: offset_px_leave,
                depth_z_px: depth_z_px_leave,
                yaw_deg: yaw_deg_leave,
            })
        } else if docked {
            None
        } else {
            // enter
            Some(CardAnimation::EdgeEnter3D {
                side: current_side,
                offset_px: offset_px_enter,
                depth_z_px: depth_z_px_enter,
                yaw_deg: yaw_deg_enter,
            })
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
        if !docked.get_untracked() {
            set_docked(true);
        }
    });

    // Error logging
    Effect::new(move |_| {
        if let Some(err) = error() {
            log::error!(
                "Error invoking {}: {err}",
                shared::commands::navigation::NAVIGATE
            );
        }
        if let Some(err) = enter_error() {
            log::error!(
                "Error invoking {}: {err}",
                shared::commands::navigation::NAV_ENTER_DONE
            );
        }
        if let Some(err) = leave_error() {
            log::error!(
                "Error invoking {}: {err}",
                shared::commands::navigation::NAV_LEAVE_DONE
            );
        }
        if let Some(err) = inset_error() {
            log::error!(
                "Error invoking {}: {err}",
                shared::commands::setup::UI_SAFE_AREA_INSETS_APPLY
            );
        }
    });

    // Derived CSS classes to prevent recalculation
    let edge_container_cls = Signal::derive(move || match placement() {
        UiPlacement::Bottom => "fixed inset-x-0 bottom-0 flex justify-center pointer-events-none",
        UiPlacement::Top => "fixed inset-x-0 top-0 flex justify-center pointer-events-none",
        UiPlacement::Left => "fixed inset-y-0 left-0 flex items-center pointer-events-none",
        UiPlacement::Right => "fixed inset-y-0 right-0 flex items-center pointer-events-none",
    });

    let card_variant = Signal::derive(move || {
        let docked = docked();
        let place = placement();
        if !docked {
            "".to_string()
        } else {
            match place {
                UiPlacement::Bottom => "rounded-b-none",
                UiPlacement::Top => "rounded-t-none",
                UiPlacement::Left => "rounded-l-none",
                UiPlacement::Right => "rounded-r-none",
            }
            .to_string()
        }
    });

    let inner_flex_class = Signal::derive(move || {
        format!(
            "flex {} gap-4 pointer-events-auto",
            if is_vertical() {
                "flex-col items-center justify-center"
            } else {
                "flex-row items-center"
            }
        )
    });

    let title_style = Signal::derive(move || {
        if is_vertical() {
            "writing-mode: vertical-rl; text-orientation: mixed;"
        } else {
            ""
        }
    });

    view! {
        <div class=edge_container_cls>
            <Card

                padding="Sm".to_string()
                class=card_variant
                start_animation=Signal::derive(menu_anim)
                on_animation_done=on_anim_done
            >
                <div class=inner_flex_class>
                    <button on:click=move |_| {
                        trigger_navigate(
                            Some(NavigateRequestPayload {
                                route: RouteId::Home,
                            }),
                        );
                    }>
                        <h1
                            class="block text-3xl italic cursor-pointer hover:underline focus:underline"
                            style=title_style
                        >
                            "Red Siren"
                        </h1>
                    </button>
                    {child_view}
                    {move || {
                        items()
                            .iter()
                            .copied()
                            .map(|item| {
                                view! {
                                    <MenuItemView
                                        item
                                        trigger_navigate
                                        compact=true
                                        menu_placement=placement()
                                    />
                                }
                            })
                            .collect_view()
                    }}
                </div>
            </Card>
        </div>
    }
}
