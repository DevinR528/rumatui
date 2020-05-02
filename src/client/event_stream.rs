use std::convert::TryFrom;
use std::sync::Arc;

use matrix_sdk::events::{
    fully_read::FullyReadEvent,
    ignored_user_list::IgnoredUserListEvent,
    presence::PresenceEvent,
    push_rules::PushRulesEvent,
    room::{
        aliases::AliasesEvent,
        avatar::AvatarEvent,
        canonical_alias::CanonicalAliasEvent,
        join_rules::JoinRulesEvent,
        member::{MemberEvent, MembershipChange, MembershipState},
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
    EventEmitter, Room,
};
use tokio::sync::mpsc;
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;

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
    },
    Message(Message, RoomId),
    FullyRead(EventId, RoomId),
    Typing(String),
    Err,
}
unsafe impl Send for StateResult {}

pub struct EventStream {
    send: Mutex<mpsc::Sender<StateResult>>,
}
unsafe impl Send for EventStream {}

impl EventStream {
    pub(crate) fn new() -> (Self, mpsc::Receiver<StateResult>) {
        let (send, recv) = mpsc::channel(1024);

        (
            Self {
                send: Mutex::new(send),
            },
            recv,
        )
    }
}

#[async_trait::async_trait]
impl EventEmitter for EventStream {
    /// Send a membership change event to the ui thread.
    async fn on_room_member(&self, room: Arc<RwLock<Room>>, event: &MemberEvent) {
        if MembershipState::Join == event.content.membership {
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
                })
                .await
            {
                panic!("{}", e)
            }
        }
    }
    /// Fires when `AsyncClient` receives a `RoomEvent::RoomName` event.
    async fn on_room_name(&self, _: Arc<RwLock<Room>>, _: &NameEvent) {}
    /// Fires when `AsyncClient` receives a `RoomEvent::RoomCanonicalAlias` event.
    async fn on_room_canonical_alias(&self, _: Arc<RwLock<Room>>, _: &CanonicalAliasEvent) {}
    /// Fires when `AsyncClient` receives a `RoomEvent::RoomAliases` event.
    async fn on_room_aliases(&self, _: Arc<RwLock<Room>>, _: &AliasesEvent) {}
    /// Fires when `AsyncClient` receives a `RoomEvent::RoomAvatar` event.
    async fn on_room_avatar(&self, _: Arc<RwLock<Room>>, _: &AvatarEvent) {}
    /// Fires when `AsyncClient` receives a `RoomEvent::RoomMessage` event.
    /// Fires when `AsyncClient` receives a `RoomEvent::RoomMessage` event.
    async fn on_room_message(&self, room: Arc<RwLock<Room>>, event: &MessageEvent) {
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
                let msg = if let Some(_fmted) = formatted_body {
                    crate::widgets::utils::markdown_to_terminal(msg_body)
                        .unwrap_or(msg_body.clone())
                } else {
                    msg_body.clone()
                };
                let txn_id = unsigned
                    .get("transaction_id")
                    .map(|id| serde_json::from_value::<String>(id.clone()).unwrap())
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
    /// Fires when `AsyncClient` receives a `RoomEvent::RoomMessageFeedback` event.
    async fn on_room_message_feedback(&self, _: Arc<RwLock<Room>>, _: &FeedbackEvent) {}
    /// Fires when `AsyncClient` receives a `RoomEvent::RoomRedaction` event.
    async fn on_room_redaction(&self, _: Arc<RwLock<Room>>, _: &RedactionEvent) {}
    /// Fires when `AsyncClient` receives a `RoomEvent::RoomPowerLevels` event.
    async fn on_room_power_levels(&self, _: Arc<RwLock<Room>>, _: &PowerLevelsEvent) {}
    /// Fires when `AsyncClient` receives a `RoomEvent::RoomTombstone` event.
    async fn on_room_tombstone(&self, _: Arc<RwLock<Room>>, _: &TombstoneEvent) {}

    // `RoomEvent`s from `IncomingState`
    /// Fires when `AsyncClient` receives a `StateEvent::RoomMember` event.
    async fn on_state_member(&self, _: Arc<RwLock<Room>>, _: &MemberEvent) {}
    /// Fires when `AsyncClient` receives a `StateEvent::RoomName` event.
    async fn on_state_name(&self, _: Arc<RwLock<Room>>, _: &NameEvent) {}
    /// Fires when `AsyncClient` receives a `StateEvent::RoomCanonicalAlias` event.
    async fn on_state_canonical_alias(&self, _: Arc<RwLock<Room>>, _: &CanonicalAliasEvent) {}
    /// Fires when `AsyncClient` receives a `StateEvent::RoomAliases` event.
    async fn on_state_aliases(&self, _: Arc<RwLock<Room>>, _: &AliasesEvent) {}
    /// Fires when `AsyncClient` receives a `StateEvent::RoomAvatar` event.
    async fn on_state_avatar(&self, _: Arc<RwLock<Room>>, _: &AvatarEvent) {}
    /// Fires when `AsyncClient` receives a `StateEvent::RoomPowerLevels` event.
    async fn on_state_power_levels(&self, _: Arc<RwLock<Room>>, _: &PowerLevelsEvent) {}
    /// Fires when `AsyncClient` receives a `StateEvent::RoomJoinRules` event.
    async fn on_state_join_rules(&self, _: Arc<RwLock<Room>>, _: &JoinRulesEvent) {}

    // `AnyStrippedStateEvent`s
    /// Fires when `AsyncClient` receives a `StateEvent::RoomMember` event.
    async fn on_stripped_state_member(&self, room: Arc<RwLock<Room>>, event: &StrippedRoomMember) {
        let StrippedRoomMember {
            sender, state_key, ..
        } = event;

        let receiver = UserId::try_from(state_key.as_str()).unwrap();
        let membership = membership_change(event);
        if let Err(e) = self
            .send
            .lock()
            .await
            .send(StateResult::Member {
                sender: sender.clone(),
                receiver,
                room,
                membership,
            })
            .await
        {
            panic!("{}", e)
        }
    }
    /// Fires when `AsyncClient` receives a `StateEvent::RoomName` event.
    async fn on_stripped_state_name(&self, _: Arc<RwLock<Room>>, _: &StrippedRoomName) {}
    /// Fires when `AsyncClient` receives a `StateEvent::RoomCanonicalAlias` event.
    async fn on_stripped_state_canonical_alias(
        &self,
        _: Arc<RwLock<Room>>,
        _: &StrippedRoomCanonicalAlias,
    ) {
    }
    /// Fires when `AsyncClient` receives a `StateEvent::RoomAliases` event.
    async fn on_stripped_state_aliases(&self, _: Arc<RwLock<Room>>, _: &StrippedRoomAliases) {}
    /// Fires when `AsyncClient` receives a `StateEvent::RoomAvatar` event.
    async fn on_stripped_state_avatar(&self, _: Arc<RwLock<Room>>, _: &StrippedRoomAvatar) {}
    /// Fires when `AsyncClient` receives a `StateEvent::RoomPowerLevels` event.
    async fn on_stripped_state_power_levels(
        &self,
        _: Arc<RwLock<Room>>,
        _: &StrippedRoomPowerLevels,
    ) {
    }
    /// Fires when `AsyncClient` receives a `StateEvent::RoomJoinRules` event.
    async fn on_stripped_state_join_rules(&self, _: Arc<RwLock<Room>>, _: &StrippedRoomJoinRules) {}

    // `NonRoomEvent` (this is a type alias from ruma_events) from `IncomingAccountData`
    /// Fires when `AsyncClient` receives a `NonRoomEvent::RoomMember` event.
    async fn on_account_presence(&self, _: Arc<RwLock<Room>>, _: &PresenceEvent) {}
    /// Fires when `AsyncClient` receives a `NonRoomEvent::RoomName` event.
    async fn on_account_ignored_users(&self, _: Arc<RwLock<Room>>, _: &IgnoredUserListEvent) {}
    /// Fires when `AsyncClient` receives a `NonRoomEvent::RoomCanonicalAlias` event.
    async fn on_account_push_rules(&self, _: Arc<RwLock<Room>>, _: &PushRulesEvent) {}
    /// Fires when `AsyncClient` receives a `NonRoomEvent::RoomAliases` event.
    async fn on_account_data_fully_read(&self, room: Arc<RwLock<Room>>, event: &FullyReadEvent) {
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
    /// Fires when `AsyncClient` receives a `NonRoomEvent::Typing` event.
    async fn on_account_data_typing(&self, room: Arc<RwLock<Room>>, event: &TypingEvent) {
        let typing = room
            .read()
            .await
            .members
            .iter()
            .filter(|(id, _)| event.content.user_ids.contains(id))
            .map(|(_, mem)| mem.name.to_string())
            .collect::<Vec<String>>();
        if let Err(e) = self
            .send
            .lock()
            .await
            .send(StateResult::Typing(if typing.is_empty() {
                String::default()
            } else {
                format!(
                    "{} {} typing...",
                    typing.join(", "),
                    if typing.len() > 1 { "are" } else { "is" }
                )
            }))
            .await
        {
            panic!("{}", e)
        }
    }

    // `PresenceEvent` is a struct so there is only the one method
    /// Fires when `AsyncClient` receives a `NonRoomEvent::RoomAliases` event.
    async fn on_presence_event(&self, _: Arc<RwLock<Room>>, _event: &PresenceEvent) {}
}

/// Helper function for membership change of StrippedRoomMember.
///
/// Check [the specification][spec] for details. [spec]: https://matrix.org/docs/spec/client_server/latest#m-room-member
pub fn membership_change(member: &StrippedRoomMember) -> MembershipChange {
    use MembershipState::*;
    let prev_membership = MembershipState::Leave;
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
        (__Nonexhaustive, _) | (_, __Nonexhaustive) => {
            panic!("__Nonexhaustive enum variant is not intended for use.")
        }
    }
}
