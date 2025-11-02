use leptos::prelude::*;
use tauri_use::{use_listen, EventType, UseListenReturn};

use crate::{
    components::{Toaster, ToasterMessage},
    util::setup_context::is_devtools_enabled,
};

#[derive(serde::Deserialize, Clone, PartialEq)]
struct LogRecord {
    message: String,
    level: u16,
}

const NOTIFICATION_CAPACITY: usize = 100;

#[component]
pub fn Notifications() -> impl IntoView {
    let UseListenReturn {
        data: log_data,
        open: log_open,
        error: log_error,
        ..
    } = use_listen::<LogRecord>(EventType::Custom("log://log"));

    let UseListenReturn {
        data: notification_data,
        open: notification_open,
        error: notification_error,
        ..
    } = use_listen::<common::events::health::NotificationPayload>(EventType::Custom(
        common::events::health::NOTIFICATION,
    ));

    let notifications = RwSignal::new(Vec::<ToasterMessage>::with_capacity(NOTIFICATION_CAPACITY));
    let on_dismiss_notification = Callback::new({
        move |id: usize| {
            notifications.update(|notes| {
                notes.retain(|note| note.id != id);
            });
        }
    });

    let is_devtools_enabled = is_devtools_enabled();

    Effect::new(move |_| {
        if is_devtools_enabled() {
            log_open();
        }
    });

    Effect::new(move |_| {
        notification_open();
    });

    Effect::new(move |_| {
        if let Some(err) = log_error() {
            log::error!("Error listening for log events: {err}");
        }

        if let Some(err) = notification_error() {
            log::error!("Error listening for notification events: {err}");
        }
    });

    Effect::new(move |_| {
        if is_devtools_enabled() {
            notifications.update(|notes| {
                notes.push(ToasterMessage::new_info(
                    "Dev tools enabled, logs and notifications will appear here!",
                ))
            });
        }
    });

    Effect::new(move |_| {
        let log_data = log_data();
        if is_devtools_enabled() {
            if let Some(LogRecord { level, message }) = log_data {
                notifications.update(|notes| {
                    match level {
                        1..=3 => notes.push(ToasterMessage::new_info(message)),
                        4..=5 => notes.push(ToasterMessage::new_warning(message)),
                        _ => notes.push(ToasterMessage::new_error(message)),
                    }
                    if notes.len() > NOTIFICATION_CAPACITY {
                        notes.remove(0);
                    }
                });
            }
        }
    });

    Effect::new(move |_| {
        let notification_data = notification_data();
        if let Some(common::events::health::NotificationPayload {
            content,
            category,
            dismiss,
        }) = notification_data
        {
            notifications.update(|notes| {
                notes.push(ToasterMessage::new(
                    content,
                    match category {
                        0 => None,
                        1..=3 => Some(crate::components::MessageType::Info),
                        4..=5 => Some(crate::components::MessageType::Warning),
                        _ => Some(crate::components::MessageType::Error),
                    },
                    if dismiss { Some(5000) } else { None },
                ));
                if notes.len() > NOTIFICATION_CAPACITY {
                    notes.remove(0);
                }
            });
        }
    });

    view! {
        <aside class="absolute top-0 right-0 left-auto z-100">
            <Toaster messages=notifications on_dismiss=on_dismiss_notification />
        </aside>
    }
}
