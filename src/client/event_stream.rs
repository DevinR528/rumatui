use std::{collections::BTreeMap, convert::TryFrom, sync::Arc};

use matrix_sdk::events::{
    fully_read::FullyReadEvent,
    ignored_user_list::IgnoredUserListEvent,
    presence::PresenceEvent,
    push_rules::PushRulesEvent,
    receipt::{ReceiptEvent, Receipts},
    room::{
        aliases::AliasesEvent,
        avatar::AvatarEvent,
        canonical_alias::CanonicalAliasEvent,
        join_rules::JoinRulesEvent,
        member::{MemberEvent, MemberEventContent, MembershipChange, MembershipState},
        message::{
            feedback::FeedbackEvent, MessageEvent, MessageEventContent, TextMessageEventContent,
        },
        name::NameEvent,
        power_levels::PowerLevelsEvent,
        redaction::RedactionEvent,
        tombstone::TombstoneEvent,
    },
    stripped::{
        StrippedRoomAliases, StrippedRoomAvatar, StrippedRoomCanonicalAlias, StrippedRoomJoinRules,
        StrippedRoomMember, StrippedRoomName, StrippedRoomPowerLevels,
    },
    typing::TypingEvent,
};
use matrix_sdk::{
    self,
    identifiers::{EventId, RoomId, UserId},
    CustomOrRawEvent, EventEmitter, Room, SyncRoom,
};
use tokio::sync::mpsc;
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;

use crate::client::ruma_ext::{
    message::EditEventContent, reaction::ReactionEventContent, ExtraMessageEventContent,
    ExtraReactionEventContent, ExtraRoomEventContent, RumaUnsupportedEvent,
};
use crate::widgets::message::Message;

/// The events sent from the `EventEmitter` are represented by this
/// enum.
///
/// Each variant represents an emitted event and is handled when sent
/// every tick of the UI loop.
pub enum StateResult {
    Member {
        sender: UserId,
        receiver: UserId,
        room: Arc<RwLock<Room>>,
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

    async fn handle_room_member(&self, room: Arc<RwLock<Room>>, event: &MemberEvent) {
        let MemberEvent {
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
            panic!("{}", e)
        }
    }
}
#[allow(clippy::eval_order_dependence)]
#[async_trait::async_trait]
impl EventEmitter for EventStream {
    /// Send a membership change event to the ui thread.
    async fn on_room_member(&self, room: SyncRoom, event: &MemberEvent) {
        match room {
            SyncRoom::Invited(room) => {
                self.handle_room_member(room, event).await;
            }
            SyncRoom::Left(room) => {
                self.handle_room_member(room, event).await;
            }
            SyncRoom::Joined(room) => {
                self.handle_room_member(room, event).await;
            }
        }
    }
    /// Fires when `AsyncClient` receives a `RoomEvent::RoomName` event.
    async fn on_room_name(&self, room: SyncRoom, _: &NameEvent) {
        if let SyncRoom::Joined(room) = room {
            if let Err(e) = self
                .send
                .lock()
                .await
                .send(StateResult::Name(
                    room.read().await.display_name(),
                    room.read().await.room_id.clone(),
                ))
                .await
            {
                panic!("{}", e)
            }
        }
    }
    /// Fires when `AsyncClient` receives a `RoomEvent::RoomCanonicalAlias` event.
    async fn on_room_canonical_alias(&self, _: SyncRoom, _: &CanonicalAliasEvent) {}
    /// Fires when `AsyncClient` receives a `RoomEvent::RoomAliases` event.
    async fn on_room_aliases(&self, _: SyncRoom, _: &AliasesEvent) {}
    /// Fires when `AsyncClient` receives a `RoomEvent::RoomAvatar` event.
    async fn on_room_avatar(&self, _: SyncRoom, _: &AvatarEvent) {}
    /// Fires when `AsyncClient` receives a `RoomEvent::RoomMessage` event.
    async fn on_room_message(&self, room: SyncRoom, event: &MessageEvent) {
        if let SyncRoom::Joined(room) = room {
            let MessageEvent {
                content,
                sender,
                event_id,
                origin_server_ts,
                unsigned,
                ..
            } = event;

            let name = if let Some(mem) = room.read().await.members.get(&sender) {
                mem.name.clone()
            } else {
                sender.localpart().into()
            };
            match content {
                MessageEventContent::Text(TextMessageEventContent {
                    body: msg_body,
                    formatted_body,
                    ..
                }) => {
                    let msg = if formatted_body.is_some() {
                        crate::widgets::utils::markdown_to_terminal(msg_body)
                            .unwrap_or(msg_body.clone())
                    } else {
                        msg_body.clone()
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
                            room.read().await.room_id.clone(),
                        ))
                        .await
                    {
                        panic!("{}", e)
                    }
                }
                _ => {}
            }
        }
    }
    /// Fires when `AsyncClient` receives a `RoomEvent::RoomMessageFeedback` event.
    async fn on_room_message_feedback(&self, _: SyncRoom, _: &FeedbackEvent) {}
    /// Fires when `AsyncClient` receives a `RoomEvent::RoomRedaction` event.
    async fn on_room_redaction(&self, room: SyncRoom, event: &RedactionEvent) {
        if let SyncRoom::Joined(room) = room {
            if let Err(e) = self
                .send
                .lock()
                .await
                .send(StateResult::Redact(
                    event.redacts.clone(),
                    room.read().await.room_id.clone(),
                ))
                .await
            {
                panic!("{}", e)
            }
        }
    }
    /// Fires when `AsyncClient` receives a `RoomEvent::RoomPowerLevels` event.
    async fn on_room_power_levels(&self, _: SyncRoom, _: &PowerLevelsEvent) {}
    /// Fires when `AsyncClient` receives a `RoomEvent::RoomTombstone` event.
    async fn on_room_tombstone(&self, _: SyncRoom, _: &TombstoneEvent) {}

    // `RoomEvent`s from `IncomingState`
    /// Fires when `AsyncClient` receives a `StateEvent::RoomMember` event.
    async fn on_state_member(&self, _: SyncRoom, _: &MemberEvent) {}
    /// Fires when `AsyncClient` receives a `StateEvent::RoomName` event.
    async fn on_state_name(&self, _: SyncRoom, _: &NameEvent) {}
    /// Fires when `AsyncClient` receives a `StateEvent::RoomCanonicalAlias` event.
    async fn on_state_canonical_alias(&self, _: SyncRoom, _: &CanonicalAliasEvent) {}
    /// Fires when `AsyncClient` receives a `StateEvent::RoomAliases` event.
    async fn on_state_aliases(&self, _: SyncRoom, _: &AliasesEvent) {}
    /// Fires when `AsyncClient` receives a `StateEvent::RoomAvatar` event.
    async fn on_state_avatar(&self, _: SyncRoom, _: &AvatarEvent) {}
    /// Fires when `AsyncClient` receives a `StateEvent::RoomPowerLevels` event.
    async fn on_state_power_levels(&self, _: SyncRoom, _: &PowerLevelsEvent) {}
    /// Fires when `AsyncClient` receives a `StateEvent::RoomJoinRules` event.
    async fn on_state_join_rules(&self, _: SyncRoom, _: &JoinRulesEvent) {}

    // `AnyStrippedStateEvent`s
    /// Fires when `AsyncClient` receives a `StateEvent::RoomMember` event.
    async fn on_stripped_state_member(
        &self,
        room: SyncRoom,
        event: &StrippedRoomMember,
        prev_content: Option<MemberEventContent>,
    ) {
        // TODO only invite is handled as stripped state member
        match room {
            SyncRoom::Invited(room) | SyncRoom::Left(room) | SyncRoom::Joined(room) => {
                let StrippedRoomMember {
                    sender, state_key, ..
                } = event;

                let receiver = UserId::try_from(state_key.as_str()).unwrap();
                let membership = stripped_membership_change(prev_content, event);
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
                    panic!("{}", e)
                }
            }
        }
    }
    /// Fires when `AsyncClient` receives a `StateEvent::RoomName` event.
    async fn on_stripped_state_name(&self, _: SyncRoom, _: &StrippedRoomName) {}
    /// Fires when `AsyncClient` receives a `StateEvent::RoomCanonicalAlias` event.
    async fn on_stripped_state_canonical_alias(&self, _: SyncRoom, _: &StrippedRoomCanonicalAlias) {
    }
    /// Fires when `AsyncClient` receives a `StateEvent::RoomAliases` event.
    async fn on_stripped_state_aliases(&self, _: SyncRoom, _: &StrippedRoomAliases) {}
    /// Fires when `AsyncClient` receives a `StateEvent::RoomAvatar` event.
    async fn on_stripped_state_avatar(&self, _: SyncRoom, _: &StrippedRoomAvatar) {}
    /// Fires when `AsyncClient` receives a `StateEvent::RoomPowerLevels` event.
    async fn on_stripped_state_power_levels(&self, _: SyncRoom, _: &StrippedRoomPowerLevels) {}
    /// Fires when `AsyncClient` receives a `StateEvent::RoomJoinRules` event.
    async fn on_stripped_state_join_rules(&self, _: SyncRoom, _: &StrippedRoomJoinRules) {}

    // `NonRoomEvent` (this is a type alias from ruma_events) from `IncomingAccountData`
    /// Fires when `AsyncClient` receives a `NonRoomEvent::RoomMember` event.
    async fn on_non_room_presence(&self, _: SyncRoom, _: &PresenceEvent) {}
    /// Fires when `AsyncClient` receives a `NonRoomEvent::RoomName` event.
    async fn on_non_room_ignored_users(&self, _: SyncRoom, _: &IgnoredUserListEvent) {}
    /// Fires when `AsyncClient` receives a `NonRoomEvent::RoomCanonicalAlias` event.
    async fn on_non_room_push_rules(&self, _: SyncRoom, _: &PushRulesEvent) {}
    /// Fires when `AsyncClient` receives a `NonRoomEvent::RoomAliases` event.
    async fn on_non_room_fully_read(&self, room: SyncRoom, event: &FullyReadEvent) {
        if let SyncRoom::Joined(room) = room {
            if let Err(e) = self
                .send
                .lock()
                .await
                .send(StateResult::FullyRead(
                    event.content.event_id.clone(),
                    room.read().await.room_id.clone(),
                ))
                .await
            {
                panic!("{}", e)
            }
        }
    }
    // TODO make the StateResult::Typing variants a list of typing users and make messages in app
    // like every other StateResult. Use Room::compute_display_name or whatever when PR is done
    /// Fires when `AsyncClient` receives a `NonRoomEvent::Typing` event.
    async fn on_non_room_typing(&self, room: SyncRoom, event: &TypingEvent) {
        if let SyncRoom::Joined(room) = room {
            let typing = room
                .read()
                .await
                .members
                .iter()
                .filter(|(id, _)| event.content.user_ids.contains(id))
                .map(|(_, mem)| mem.name.to_string())
                .collect::<Vec<String>>();
            let room_id = room.read().await.room_id.clone();
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
                panic!("{}", e)
            }
        }
    }

    async fn on_non_room_receipt(&self, room: SyncRoom, event: &ReceiptEvent) {
        if let SyncRoom::Joined(room) = room {
            let room_id = room.read().await.room_id.clone();
            let events = event.content.clone();
            if let Err(e) = self
                .send
                .lock()
                .await
                .send(StateResult::ReadReceipt(room_id, events))
                .await
            {
                panic!("{}", e)
            }
        }
    }

    /// Fires when `AsyncClient` receives a `PresenceEvent` event.
    async fn on_presence_event(&self, _: SyncRoom, _event: &PresenceEvent) {}

    async fn on_unrecognized_event(&self, room: SyncRoom, event: &CustomOrRawEvent<'_>) {
        match room {
            SyncRoom::Joined(room) => {
                match event {
                    CustomOrRawEvent::RawJson(raw) => {
                        if let Ok(event) = serde_json::from_str::<RumaUnsupportedEvent>(raw.get()) {
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
                                            let new_body = if new_content.formatted_body.is_some() {
                                                crate::widgets::utils::markdown_to_terminal(&body)
                                                    // this shouldn't fail but as a back up we just use
                                                    // the unformatted message body
                                                    .unwrap_or(body.clone())
                                            } else {
                                                body.to_string()
                                            };
                                            let event_id = relates_to.event_id.clone();
                                            let room_id = room.read().await.room_id.clone();
                                            if let Err(e) = self
                                                .send
                                                .lock()
                                                .await
                                                .send(StateResult::MessageEdit(
                                                    new_body, room_id, event_id,
                                                ))
                                                .await
                                            {
                                                panic!("{}", e)
                                            }
                                        }
                                    }
                                },
                                ExtraRoomEventContent::Reaction { content: _ } => {}
                            }
                        }
                    }
                    CustomOrRawEvent::CustomRoom(room_event) => {
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
                                        let room_id = room.read().await.room_id.clone();
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
                                            panic!("{}", e)
                                        }
                                    }
                                }
                            }
                        }
                    }
                    CustomOrRawEvent::CustomState(_state_event) => {}
                    CustomOrRawEvent::Custom(_event) => {}
                }
            }
            SyncRoom::Left(_room) => {}
            _ => {}
        }
    }

    // // `RumaUnsupportedEvent
    // /// Fires when `Client` receives a `RumaUnsupportedRoomEvent<ExtraRoomEventContent::Reaction>`.
    // async fn on_reaction_event(&self, room: SyncRoom, event: &ExtraReactionEventContent) {
    //     if let SyncRoom::Joined(room) = room {
    //         let ReactionEventContent::Annotation { event_id, key } = &event.relates_to;
    //         let event_id = event_id.clone();
    //         let room_id = room.read().await.room_id.clone();
    //         if let Err(e) = self
    //             .send
    //             .lock()
    //             .await
    //             .send(StateResult::Reaction(room_id, event_id, key.to_string()))
    //             .await
    //         {
    //             panic!("{}", e)
    //         }
    //     }
    // }
    // /// Fires when `Client` receives a `RumaUnsupportedRoomEvent<ExtraRoomEventContent::MessageEdit>`.
    // async fn on_message_edit_event(&self, room: SyncRoom, event: &ExtraMessageEventContent) {
    //     if let SyncRoom::Joined(room) = room {
    //         let ExtraMessageEventContent::EditEvent(edit) = event;
    //         if edit.new_content.msgtype == "m.text" && edit.relates_to.rel_type == "m.replace" {
    //             let new_body = if let Some(fmt) = edit.new_content.formatted_body.as_ref() {
    //                 fmt.to_string()
    //             } else {
    //                 edit.body.to_string()
    //             };
    //             let event_id = edit.relates_to.event_id.clone();
    //             let room_id = room.read().await.room_id.clone();
    //             if let Err(e) = self
    //                 .send
    //                 .lock()
    //                 .await
    //                 .send(StateResult::MessageEdit(new_body, room_id, event_id))
    //                 .await
    //             {
    //                 panic!("{}", e)
    //             }
    //         }
    //     }
    // }
}

/// Helper function for membership change of StrippedRoomMember.
///
/// Check [the specification][spec] for details. [spec]: https://matrix.org/docs/spec/client_server/latest#m-room-member
pub fn stripped_membership_change(
    prev_content: Option<MemberEventContent>,
    member: &StrippedRoomMember,
) -> MembershipChange {
    use MembershipState::*;

    let prev_membership = if let Some(prev) = &prev_content {
        prev.membership
    } else {
        Leave
    };
    match (prev_membership, &member.content.membership) {
        (Invite, Invite) | (Leave, Leave) | (Ban, Ban) => MembershipChange::None,
        (Invite, Join) | (Leave, Join) => MembershipChange::Joined,
        (Invite, Leave) => {
            if member.sender == member.state_key {
                MembershipChange::InvitationRevoked
            } else {
                MembershipChange::InvitationRejected
            }
        }
        (Invite, Ban) | (Leave, Ban) => MembershipChange::Banned,
        (Join, Invite) | (Ban, Invite) | (Ban, Join) => MembershipChange::Error,
        (Join, Join) => MembershipChange::ProfileChanged,
        (Join, Leave) => {
            if member.sender == member.state_key {
                MembershipChange::Left
            } else {
                MembershipChange::Kicked
            }
        }
        (Join, Ban) => MembershipChange::KickedAndBanned,
        (Leave, Invite) => MembershipChange::Invited,
        (Ban, Leave) => MembershipChange::Unbanned,
        (Knock, _) | (_, Knock) => MembershipChange::NotImplemented,
    }
}
