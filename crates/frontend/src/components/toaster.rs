use std::{sync::atomic::AtomicUsize, time::Duration};

use leptos::prelude::*;
use web_sys::MouseEvent;

use crate::components::{Button, Card, Icon, Tooltip, UiPadding, UiPlacement, UiSize, UiVariant};

static TOASTER_MESSAGE_ID: AtomicUsize = AtomicUsize::new(1);

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MessageType {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToasterMessage {
    pub id: usize,
    pub content: String,
    pub message_type: Option<MessageType>,
    pub duration_ms: Option<u64>,
}

impl ToasterMessage {
    pub fn new(
        content: impl Into<String>,
        message_type: Option<MessageType>,
        duration_ms: Option<u64>,
    ) -> Self {
        let id = TOASTER_MESSAGE_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Self {
            id,
            content: content.into(),
            message_type,
            duration_ms,
        }
    }

    pub fn new_error(content: impl Into<String>) -> Self {
        Self::new(content, Some(MessageType::Error), None)
    }

    pub fn new_warning(content: impl Into<String>) -> Self {
        Self::new(content, Some(MessageType::Warning), None)
    }

    pub fn new_info(content: impl Into<String>) -> Self {
        Self::new(content, Some(MessageType::Info), None)
    }
}

#[component]
pub fn Toaster(
    #[prop(into)] messages: Signal<Vec<ToasterMessage>>,
    #[prop(into)] on_dismiss: Callback<usize>,
) -> impl IntoView {
    let (current_message_id, set_current_message_id) =
        signal(messages.get_untracked().last().map(|m| m.id));

    Effect::new(move || {
        if let Some(last_msg) = messages.get().last() {
            set_current_message_id(Some(last_msg.id));
            if let Some(duration_ms) = last_msg.duration_ms {
                let msg_id = last_msg.id;
                set_timeout(
                    move || {
                        on_dismiss.run(msg_id);
                    },
                    Duration::from_millis(duration_ms),
                );
            }
        } else {
            set_current_message_id(None);
        }
    });

    let current_message = Signal::derive(move || {
        let messages = messages();
        let current_message_id = current_message_id()?;
        messages
            .iter()
            .find(|m| m.id == current_message_id)
            .cloned()
    });

    let next = Signal::derive(move || {
        let messages = messages();
        messages
            .iter()
            .position(|m| Some(m.id) == current_message_id())
            .and_then(|pos| {
                if pos + 1 < messages.len() {
                    Some(messages[pos + 1].id)
                } else {
                    None
                }
            })
    });

    let previous = Signal::derive(move || {
        let messages = messages();
        messages
            .iter()
            .position(|m| Some(m.id) == current_message_id())
            .and_then(|pos| {
                if pos >= 1 {
                    Some(messages[pos - 1].id)
                } else {
                    None
                }
            })
    });

    let on_cancel = move |e: MouseEvent| {
        if e.ctrl_key() || e.meta_key() || e.shift_key() {
            messages().iter().for_each(|m| on_dismiss.run(m.id));
        } else if let Some(m) = current_message() {
            on_dismiss.run(m.id)
        }
    };

    view! {
        <Show when=move || current_message().is_some()>
            <Card
                card_animation_direction=UiPlacement::Left
                padding=UiPadding::Sm
                variant=UiVariant::Outline
                class="rounded-r-none border-r-0 max-w-full md:max-w-96 w-full sm:w-96 overflow-hidden h-auto! bg-red! dark:bg-black!"
            >
                {move || {
                    let current_message = current_message().unwrap();
                    let message_type = current_message.message_type;
                    view! {
                        <div class="flex flex-col items-stretch gap-2 text-sm pr-3">
                            <div class="flex gap-1 justify-end">
                                <Tooltip text="Previous" placement=UiPlacement::Bottom>
                                    <Button
                                        variant=UiVariant::Ghost
                                        size=UiSize::Sm
                                        on:click=move |_| {
                                            if let Some(prev_id) = previous() {
                                                set_current_message_id(Some(prev_id));
                                            }
                                        }
                                        disabled=Signal::derive(move || previous().is_none())
                                    >
                                        <Icon size=UiSize::Sm name="chevron-left" />
                                    </Button>
                                </Tooltip>
                                <Tooltip text="Close" placement=UiPlacement::Bottom>
                                    <Button
                                        variant=UiVariant::Ghost
                                        size=UiSize::Sm
                                        on:click=on_cancel
                                    >
                                        <Icon size=UiSize::Sm name="cancel" />
                                    </Button>
                                </Tooltip>
                                <Tooltip text="Next" placement=UiPlacement::Bottom>
                                    <Button
                                        variant=UiVariant::Ghost
                                        size=UiSize::Sm
                                        on:click=move |_| {
                                            if let Some(next_id) = next() {
                                                set_current_message_id(Some(next_id));
                                            }
                                        }
                                        disabled=Signal::derive(move || next().is_none())
                                    >
                                        <Icon size=UiSize::Sm name="chevron-right" />
                                    </Button>
                                </Tooltip>
                            </div>
                            <div class="flex items-start gap-4">
                                <Show when=move || message_type.is_some()>
                                    <div class="shrink-0">
                                        <Icon name=match message_type.unwrap() {
                                            MessageType::Error => "skull",
                                            MessageType::Warning => "warning",
                                            MessageType::Info => "ok",
                                        } />
                                    </div>
                                </Show>
                                <div class="select-text min-w-0">
                                    <p class="break-words whitespace-pre-wrap">
                                        {current_message.content}
                                    </p>
                                </div>
                            </div>
                        </div>
                    }
                }}
            </Card>
        </Show>
    }
}
