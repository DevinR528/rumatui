use std::{collections::BTreeMap, convert::TryFrom, sync::Arc};

use matrix_sdk::{
    self,
    events::{
        fully_read::FullyReadEventContent,
        ignored_user_list::IgnoredUserListEventContent,
        presence::PresenceEvent,
        push_rules::PushRulesEventContent,
        receipt::{ReceiptEventContent, Receipts},
        room::{
            aliases::AliasesEventContent,
            avatar::AvatarEventContent,
            canonical_alias::CanonicalAliasEventContent,
            join_rules::JoinRulesEventContent,
            member::{MemberEventContent, MembershipChange},
            message::{
                feedback::FeedbackEventContent, MessageEventContent, TextMessageEventContent,
            },
            name::NameEventContent,
            power_levels::PowerLevelsEventContent,
            redaction::SyncRedactionEvent,
            tombstone::TombstoneEventContent,
        },
        typing::TypingEventContent,
        BasicEvent, StrippedStateEvent, SyncEphemeralRoomEvent, SyncMessageEvent, SyncStateEvent,
    },
    identifiers::{EventId, RoomId, UserId},
    CustomEvent, EventEmitter, RoomState,
};
use serde_json::value::RawValue as RawJsonValue;

use tokio::sync::{mpsc, Mutex};
use uuid::Uuid;

use crate::{
    client::ruma_ext::{
        message::EditEventContent, reaction::ReactionEventContent, ExtraMessageEventContent,
        ExtraReactionEventContent, ExtraRoomEventContent, RumaUnsupportedEvent,
    },
    widgets::message::Message,
};

/// The events sent from the `EventEmitter` are represented by this
/// enum.
///
/// Each variant represents an emitted event and is handled when sent
/// every tick of the UI loop.
pub enum StateResult {
    Member {
        sender: UserId,
        receiver: UserId,
        room: RoomState,
        membership: MembershipChange,
        timeline_event: bool,
    },
    Message(Message, RoomId),
    MessageEdit(String, RoomId, EventId),
    Name(String, RoomId),
    FullyRead(EventId, RoomId),
    ReadReceipt(RoomId, BTreeMap<EventId, Receipts>),
    Reaction(EventId, EventId, RoomId, String),
    Redact(EventId, RoomId),
    Typing(RoomId, String),
    Err,
}
unsafe impl Send for StateResult {}

#[derive(Clone, Debug)]
pub struct EventStream {
    /// Send messages to the UI loop.
    send: Arc<Mutex<mpsc::Sender<StateResult>>>,
}
unsafe impl Send for EventStream {}

impl EventStream {
    pub(crate) fn new() -> (Self, mpsc::Receiver<StateResult>) {
        let (send, recv) = mpsc::channel(1024);

        (
            Self {
                send: Arc::new(Mutex::new(send)),
            },
            recv,
        )
    }

    async fn handle_room_member(
        &self,
        room: RoomState,
        event: &SyncStateEvent<MemberEventContent>,
    ) {
        let SyncStateEvent {
            sender, state_key, ..
        } = event;
        let receiver = UserId::try_from(state_key.as_str()).unwrap();
        let membership = event.membership_change();
        if let Err(e) = self
            .send
            .lock()
            .await
            .send(StateResult::Member {
                sender: sender.clone(),
                receiver,
                room,
                membership,
                timeline_event: true,
            })
            .await
        {
            tracing::error!("event stream channel closed {}", e);
            panic!("{}", e)
        }
    }
}
#[allow(clippy::eval_order_dependence)]
#[async_trait::async_trait]
impl EventEmitter for EventStream {
    /// Send a membership change event to the ui thread.
    async fn on_room_member(&self, room: RoomState, event: &SyncStateEvent<MemberEventContent>) {
        self.handle_room_member(room, event).await;
    }
    /// Fires when `AsyncClient` receives a `RoomEvent::RoomName` event.
    async fn on_room_name(&self, room: RoomState, _: &SyncStateEvent<NameEventContent>) {
        if let RoomState::Joined(room) = room {
            if let Err(e) = self
                .send
                .lock()
                .await
                .send(StateResult::Name(
                    room.display_name().await.unwrap(),
                    room.room_id().clone(),
                ))
                .await
            {
                tracing::error!("event stream channel closed {}", e);
                panic!("{}", e)
            }
        }
    }
    /// Fires when `AsyncClient` receives a `RoomEvent::RoomCanonicalAlias` event.
    async fn on_room_canonical_alias(
        &self,
        _: RoomState,
        _: &SyncStateEvent<CanonicalAliasEventContent>,
    ) {
    }
    /// Fires when `AsyncClient` receives a `RoomEvent::RoomAliases` event.
    async fn on_room_aliases(&self, _: RoomState, _: &SyncStateEvent<AliasesEventContent>) {}
    /// Fires when `AsyncClient` receives a `RoomEvent::RoomAvatar` event.
    async fn on_room_avatar(&self, _: RoomState, _: &SyncStateEvent<AvatarEventContent>) {}
    /// Fires when `AsyncClient` receives a `RoomEvent::RoomMessage` event.
    async fn on_room_message(
        &self,
        room: RoomState,
        event: &SyncMessageEvent<MessageEventContent>,
    ) {
        if let RoomState::Joined(room) = room {
            let SyncMessageEvent {
                content,
                sender,
                event_id,
                origin_server_ts,
                unsigned,
                ..
            } = event;

            let name = if let Some(mem) = room.get_member(&sender).await.unwrap() {
                mem.name().to_string()
            } else {
                sender.localpart().into()
            };
            match content {
                MessageEventContent::Text(TextMessageEventContent {
                    body, formatted, ..
                }) => {
                    let msg = if formatted
                        .as_ref()
                        .map(|f| f.body.to_string())
                        .unwrap_or(body.to_string())
                        != *body
                    {
                        // This is extremely expensive
                        // TODO cache these results somehow
                        crate::widgets::utils::markdown_to_terminal(body).unwrap_or(body.clone())
                    } else {
                        body.clone()
                    };
                    let txn_id = unsigned
                        .transaction_id
                        .as_ref()
                        .cloned()
                        .unwrap_or_default();

                    if let Err(e) = self
                        .send
                        .lock()
                        .await
                        .send(StateResult::Message(
                            Message {
                                name,
                                user: sender.clone(),
                                text: msg,
                                event_id: event_id.clone(),
                                timestamp: *origin_server_ts,
                                uuid: Uuid::parse_str(&txn_id).unwrap_or(Uuid::new_v4()),
                                read: false,
                                reactions: vec![],
                                sent_receipt: false,
                            },
                            room.room_id().clone(),
                        ))
                        .await
                    {
                        tracing::error!("event stream channel closed {}", e);
                        panic!("{}", e)
                    }
                }
                _ => {}
            }
        }
    }
    /// Fires when `AsyncClient` receives a `RoomEvent::RoomMessageFeedback` event.
    async fn on_room_message_feedback(
        &self,
        _: RoomState,
        _: &SyncMessageEvent<FeedbackEventContent>,
    ) {
    }
    /// Fires when `AsyncClient` receives a `RoomEvent::RoomRedaction` event.
    async fn on_room_redaction(&self, room: RoomState, event: &SyncRedactionEvent) {
        if let RoomState::Joined(room) = room {
            if let Err(e) = self
                .send
                .lock()
                .await
                .send(StateResult::Redact(
                    event.redacts.clone(),
                    room.room_id().clone(),
                ))
                .await
            {
                tracing::error!("event stream channel closed {}", e);
                panic!("{}", e)
            }
        }
    }
    /// Fires when `AsyncClient` receives a `RoomEvent::RoomPowerLevels` event.
    async fn on_room_power_levels(
        &self,
        _: RoomState,
        _: &SyncStateEvent<PowerLevelsEventContent>,
    ) {
    }
    /// Fires when `AsyncClient` receives a `RoomEvent::RoomTombstone` event.
    async fn on_room_tombstone(&self, _: RoomState, _: &SyncStateEvent<TombstoneEventContent>) {}

    // `RoomEvent`s from `IncomingState`
    /// Fires when `AsyncClient` receives a `StateEvent::RoomMember` event.
    async fn on_state_member(&self, _: RoomState, _: &SyncStateEvent<MemberEventContent>) {}
    /// Fires when `AsyncClient` receives a `StateEvent::RoomName` event.
    async fn on_state_name(&self, _: RoomState, _: &SyncStateEvent<NameEventContent>) {}
    /// Fires when `AsyncClient` receives a `StateEvent::RoomCanonicalAlias` event.
    async fn on_state_canonical_alias(
        &self,
        _: RoomState,
        _: &SyncStateEvent<CanonicalAliasEventContent>,
    ) {
    }
    /// Fires when `AsyncClient` receives a `StateEvent::RoomAliases` event.
    async fn on_state_aliases(&self, _: RoomState, _: &SyncStateEvent<AliasesEventContent>) {}
    /// Fires when `AsyncClient` receives a `StateEvent::RoomAvatar` event.
    async fn on_state_avatar(&self, _: RoomState, _: &SyncStateEvent<AvatarEventContent>) {}
    /// Fires when `AsyncClient` receives a `StateEvent::RoomPowerLevels` event.
    async fn on_state_power_levels(
        &self,
        _: RoomState,
        _: &SyncStateEvent<PowerLevelsEventContent>,
    ) {
    }
    /// Fires when `AsyncClient` receives a `StateEvent::RoomJoinRules` event.
    async fn on_state_join_rules(&self, _: RoomState, _: &SyncStateEvent<JoinRulesEventContent>) {}

    // `AnyStrippedStateEvent`s
    /// Fires when `AsyncClient` receives a `StateEvent::RoomMember` event.
    async fn on_stripped_state_member(
        &self,
        room: RoomState,
        event: &StrippedStateEvent<MemberEventContent>,
        _prev_content: Option<MemberEventContent>,
    ) {
        // TODO only invite is handled as stripped state member
        let StrippedStateEvent {
            sender, state_key, ..
        } = event;

        let receiver = UserId::try_from(state_key.as_str()).unwrap();
        let membership = event.membership_change();
        if let Err(e) = self
            .send
            .lock()
            .await
            .send(StateResult::Member {
                sender: sender.clone(),
                receiver,
                room,
                membership,
                timeline_event: false,
            })
            .await
        {
            tracing::error!("event stream channel closed {}", e);
            panic!("{}", e)
        }
    }
    /// Fires when `AsyncClient` receives a `StateEvent::RoomName` event.
    async fn on_stripped_state_name(&self, _: RoomState, _: &StrippedStateEvent<NameEventContent>) {
    }
    /// Fires when `AsyncClient` receives a `StateEvent::RoomCanonicalAlias` event.
    async fn on_stripped_state_canonical_alias(
        &self,
        _: RoomState,
        _: &StrippedStateEvent<CanonicalAliasEventContent>,
    ) {
    }
    /// Fires when `AsyncClient` receives a `StateEvent::RoomAliases` event.
    async fn on_stripped_state_aliases(
        &self,
        _: RoomState,
        _: &StrippedStateEvent<AliasesEventContent>,
    ) {
    }
    /// Fires when `AsyncClient` receives a `StateEvent::RoomAvatar` event.
    async fn on_stripped_state_avatar(
        &self,
        _: RoomState,
        _: &StrippedStateEvent<AvatarEventContent>,
    ) {
    }
    /// Fires when `AsyncClient` receives a `StateEvent::RoomPowerLevels` event.
    async fn on_stripped_state_power_levels(
        &self,
        _: RoomState,
        _: &StrippedStateEvent<PowerLevelsEventContent>,
    ) {
    }
    /// Fires when `AsyncClient` receives a `StateEvent::RoomJoinRules` event.
    async fn on_stripped_state_join_rules(
        &self,
        _: RoomState,
        _: &StrippedStateEvent<JoinRulesEventContent>,
    ) {
    }

    // `NonRoomEvent` (this is a type alias from ruma_events) from `IncomingAccountData`
    /// Fires when `AsyncClient` receives a `NonRoomEvent::RoomMember` event.
    async fn on_non_room_presence(&self, _: RoomState, _: &PresenceEvent) {}
    /// Fires when `AsyncClient` receives a `NonRoomEvent::RoomName` event.
    async fn on_non_room_ignored_users(
        &self,
        _: RoomState,
        _: &BasicEvent<IgnoredUserListEventContent>,
    ) {
    }
    /// Fires when `AsyncClient` receives a `NonRoomEvent::RoomCanonicalAlias` event.
    async fn on_non_room_push_rules(&self, _: RoomState, _: &BasicEvent<PushRulesEventContent>) {}
    /// Fires when `AsyncClient` receives a `NonRoomEvent::RoomAliases` event.
    async fn on_non_room_fully_read(
        &self,
        room: RoomState,
        event: &SyncEphemeralRoomEvent<FullyReadEventContent>,
    ) {
        if let RoomState::Joined(room) = room {
            if let Err(e) = self
                .send
                .lock()
                .await
                .send(StateResult::FullyRead(
                    event.content.event_id.clone(),
                    room.room_id().clone(),
                ))
                .await
            {
                tracing::error!("event stream channel closed {}", e);
                panic!("{}", e)
            }
        }
    }

    // TODO make the StateResult::Typing variants a list of typing users and make messages in app
    // like every other StateResult. Use Room::compute_display_name or whatever when PR is done
    /// Fires when `AsyncClient` receives a `NonRoomEvent::Typing` event.
    async fn on_non_room_typing(
        &self,
        room: RoomState,
        event: &SyncEphemeralRoomEvent<TypingEventContent>,
    ) {
        if let RoomState::Joined(room) = room {
            let typing = room
                .joined_members()
                .await
                .unwrap()
                .iter()
                .filter(|mem| event.content.user_ids.contains(mem.user_id()))
                .map(|mem| mem.name().to_string())
                .collect::<Vec<String>>();
            let room_id = room.room_id().clone();
            let notice = if typing.is_empty() {
                String::default()
            } else {
                format!(
                    "{} {} typing...",
                    typing.join(", "),
                    if typing.len() > 1 { "are" } else { "is" }
                )
            };
            if let Err(e) = self
                .send
                .lock()
                .await
                .send(StateResult::Typing(room_id, notice))
                .await
            {
                tracing::error!("event stream channel closed {}", e);
                panic!("{}", e)
            }
        }
    }

    async fn on_non_room_receipt(
        &self,
        room: RoomState,
        event: &SyncEphemeralRoomEvent<ReceiptEventContent>,
    ) {
        if let RoomState::Joined(room) = room {
            let room_id = room.room_id().clone();
            let events = event.content.0.clone();
            if let Err(e) = self
                .send
                .lock()
                .await
                .send(StateResult::ReadReceipt(room_id, events))
                .await
            {
                tracing::error!("event stream channel closed {}", e);
                panic!("{}", e)
            }
        }
    }

    /// Fires when `AsyncClient` receives a `PresenceEvent` event.
    async fn on_presence_event(&self, _event: &PresenceEvent) {}

    async fn on_unrecognized_event(&self, room: RoomState, event: &RawJsonValue) {
        match room {
            RoomState::Joined(room) => {
                if let Ok(event) = serde_json::from_str::<RumaUnsupportedEvent>(event.get()) {
                    match event.content {
                        ExtraRoomEventContent::Message { content } => match content {
                            ExtraMessageEventContent::EditEvent(EditEventContent {
                                body,
                                new_content,
                                relates_to,
                            }) => {
                                if new_content.msgtype == "m.text"
                                    && relates_to.rel_type == "m.replace"
                                {
                                    let new_body = if body.contains('`') {
                                        crate::widgets::utils::markdown_to_terminal(&body)
                                            // this shouldn't fail but as a back up we just use
                                            // the unformatted message body
                                            .unwrap_or(body.clone())
                                    // None.unwrap_or(body.clone())
                                    } else {
                                        body.to_string()
                                    };
                                    let event_id = relates_to.event_id.clone();
                                    let room_id = room.room_id().clone();
                                    if let Err(e) = self
                                        .send
                                        .lock()
                                        .await
                                        .send(StateResult::MessageEdit(new_body, room_id, event_id))
                                        .await
                                    {
                                        tracing::error!("event stream channel closed {}", e);
                                        panic!("{}", e)
                                    }
                                }
                            }
                        },
                        ExtraRoomEventContent::Reaction { content: _ } => {}
                    }
                }
            }
            RoomState::Left(_room) => {}
            _ => {}
        }
    }

    async fn on_custom_event(&self, room: RoomState, event: &CustomEvent<'_>) {
        match room {
            RoomState::Joined(room) => {
                match event {
                    CustomEvent::Message(room_event) => {
                        if let Ok(raw) = serde_json::value::to_raw_value(room_event) {
                            // TODO this is dumb don't deserialize then serialize but this should all
                            // be removed once ruma_events 0.22 is released
                            if let Ok(event) =
                                serde_json::from_str::<RumaUnsupportedEvent>(raw.get())
                            {
                                match event.content {
                                    ExtraRoomEventContent::Message { content: _ } => {}
                                    ExtraRoomEventContent::Reaction {
                                        content:
                                            ExtraReactionEventContent {
                                                relates_to:
                                                    ReactionEventContent::Annotation { event_id, key },
                                            },
                                    } => {
                                        let event_id = event_id.clone();
                                        let room_id = room.room_id().clone();
                                        if let Err(e) = self
                                            .send
                                            .lock()
                                            .await
                                            .send(StateResult::Reaction(
                                                event_id,
                                                event.event_id.clone(),
                                                room_id,
                                                key.to_string(),
                                            ))
                                            .await
                                        {
                                            tracing::error!("event stream channel closed {}", e);
                                            panic!("{}", e)
                                        }
                                    }
                                }
                            }
                        }
                    }
                    CustomEvent::State(_state_event) => {}
                    CustomEvent::Basic(_event) => {}
                    CustomEvent::EphemeralRoom(_event) => {}
                    CustomEvent::StrippedState(_event) => {}
                }
            }
            RoomState::Left(_room) => {}
            _ => {}
        }
    }
}
