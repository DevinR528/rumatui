use std::ops::Deref;
use std::sync::Arc;

use matrix_sdk::{
    self,
    identifiers::{RoomId},
    EventEmitter, Room,
};

use matrix_sdk::events::{
    // call::{
    //     answer::AnswerEvent, candidates::CandidatesEvent, hangup::HangupEvent, invite::InviteEvent,
    // },
    // direct::DirectEvent,
    // dummy::DummyEvent,
    // forwarded_room_key::ForwardedRoomKeyEvent,
    fully_read::FullyReadEvent,
    ignored_user_list::IgnoredUserListEvent,
    // key::verification::{
    //     accept::AcceptEvent, cancel::CancelEvent, key::KeyEvent, mac::MacEvent,
    //     request::RequestEvent, start::StartEvent,
    // },
    presence::PresenceEvent,
    push_rules::PushRulesEvent,
    // receipt::ReceiptEvent,
    room::{
        aliases::AliasesEvent,
        avatar::AvatarEvent,
        canonical_alias::CanonicalAliasEvent,
        // create::CreateEvent,
        // encrypted::EncryptedEvent,
        // encryption::EncryptionEvent,
        // guest_access::GuestAccessEvent,
        // history_visibility::HistoryVisibilityEvent,
        join_rules::JoinRulesEvent,
        member::MemberEvent,
        message::{
            feedback::FeedbackEvent, MessageEvent, MessageEventContent, TextMessageEventContent,
        },
        name::NameEvent,
        // pinned_events::PinnedEventsEvent,
        power_levels::PowerLevelsEvent,
        redaction::RedactionEvent,
        // server_acl::ServerAclEvent,
        // third_party_invite::ThirdPartyInviteEvent,
        // tombstone::TombstoneEvent,
        // topic::TopicEvent,
    },
    // room_key::RoomKeyEvent,
    // room_key_request::RoomKeyRequestEvent,
    // sticker::StickerEvent,
    // tag::TagEvent,
    // typing::TypingEvent,
    // CustomEvent, CustomRoomEvent, CustomStateEvent,
};

use tokio::sync::mpsc;
use tokio::sync::Mutex;

pub enum StateResult {
    Message(crate::UserIdStr, String, RoomId),
    Err,
}
unsafe impl Send for StateResult {}

pub struct EventStream {
    send: mpsc::Sender<StateResult>,
}
unsafe impl Send for EventStream {}

impl EventStream {
    pub(crate) fn new() -> (Self, mpsc::Receiver<StateResult>) {
        let (send, recv) = mpsc::channel(1024);

        (Self { send }, recv)
    }
}

#[async_trait::async_trait]
impl EventEmitter for EventStream {
    async fn on_room_member(&mut self, _: Arc<Mutex<Room>>, _: Arc<Mutex<MemberEvent>>) {}
    /// Fires when `AsyncClient` receives a `RoomEvent::RoomName` event.
    async fn on_room_name(&mut self, _: Arc<Mutex<Room>>, _: Arc<Mutex<NameEvent>>) {}
    /// Fires when `AsyncClient` receives a `RoomEvent::RoomCanonicalAlias` event.
    async fn on_room_canonical_alias(
        &mut self,
        _: Arc<Mutex<Room>>,
        _: Arc<Mutex<CanonicalAliasEvent>>,
    ) {
    }
    /// Fires when `AsyncClient` receives a `RoomEvent::RoomAliases` event.
    async fn on_room_aliases(&mut self, _: Arc<Mutex<Room>>, _: Arc<Mutex<AliasesEvent>>) {}
    /// Fires when `AsyncClient` receives a `RoomEvent::RoomAvatar` event.
    async fn on_room_avatar(&mut self, _: Arc<Mutex<Room>>, _: Arc<Mutex<AvatarEvent>>) {}
    /// Fires when `AsyncClient` receives a `RoomEvent::RoomMessage` event.
    /// Fires when `AsyncClient` receives a `RoomEvent::RoomMessage` event.
    async fn on_room_message(&mut self, room: Arc<Mutex<Room>>, event: Arc<Mutex<MessageEvent>>) {
        let r = room.lock().await;
        let Room {
            room_id, members, ..
        } = r.deref();
        let ev = event.lock().await;
        let ev = ev.deref();

        let MessageEvent {
            content, sender, ..
        } = ev;

        let name = if let Some(mem) = members.get(&sender) {
            mem.name.clone()
        } else {
            sender.localpart().into()
        };
        match content {
            MessageEventContent::Text(TextMessageEventContent { body: msg_body, formatted_body, .. }) => {
                let msg = if let Some(_fmted) = formatted_body {
                    crate::widgets::utils::markdown_to_terminal(msg_body).unwrap_or(msg_body.clone())
                } else {
                    msg_body.clone()
                };
                if let Err(e) = self
                    .send
                    .send(StateResult::Message(
                        name,
                        msg,
                        room_id.clone(),
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
    async fn on_room_message_feedback(
        &mut self,
        _: Arc<Mutex<Room>>,
        _: Arc<Mutex<FeedbackEvent>>,
    ) {
    }
    /// Fires when `AsyncClient` receives a `RoomEvent::RoomRedaction` event.
    async fn on_room_redaction(&mut self, _: Arc<Mutex<Room>>, _: Arc<Mutex<RedactionEvent>>) {}
    /// Fires when `AsyncClient` receives a `RoomEvent::RoomPowerLevels` event.
    async fn on_room_power_levels(&mut self, _: Arc<Mutex<Room>>, _: Arc<Mutex<PowerLevelsEvent>>) {
    }

    // `RoomEvent`s from `IncomingState`
    /// Fires when `AsyncClient` receives a `StateEvent::RoomMember` event.
    async fn on_state_member(&mut self, _: Arc<Mutex<Room>>, _: Arc<Mutex<MemberEvent>>) {}
    /// Fires when `AsyncClient` receives a `StateEvent::RoomName` event.
    async fn on_state_name(&mut self, _: Arc<Mutex<Room>>, _: Arc<Mutex<NameEvent>>) {}
    /// Fires when `AsyncClient` receives a `StateEvent::RoomCanonicalAlias` event.
    async fn on_state_canonical_alias(
        &mut self,
        _: Arc<Mutex<Room>>,
        _: Arc<Mutex<CanonicalAliasEvent>>,
    ) {
    }
    /// Fires when `AsyncClient` receives a `StateEvent::RoomAliases` event.
    async fn on_state_aliases(&mut self, _: Arc<Mutex<Room>>, _: Arc<Mutex<AliasesEvent>>) {}
    /// Fires when `AsyncClient` receives a `StateEvent::RoomAvatar` event.
    async fn on_state_avatar(&mut self, _: Arc<Mutex<Room>>, _: Arc<Mutex<AvatarEvent>>) {}
    /// Fires when `AsyncClient` receives a `StateEvent::RoomPowerLevels` event.
    async fn on_state_power_levels(
        &mut self,
        _: Arc<Mutex<Room>>,
        _: Arc<Mutex<PowerLevelsEvent>>,
    ) {
    }
    /// Fires when `AsyncClient` receives a `StateEvent::RoomJoinRules` event.
    async fn on_state_join_rules(&mut self, _: Arc<Mutex<Room>>, _: Arc<Mutex<JoinRulesEvent>>) {}

    // `NonRoomEvent` (this is a type alias from ruma_events) from `IncomingAccountData`
    /// Fires when `AsyncClient` receives a `NonRoomEvent::RoomMember` event.
    async fn on_account_presence(&mut self, _: Arc<Mutex<Room>>, _: Arc<Mutex<PresenceEvent>>) {}
    /// Fires when `AsyncClient` receives a `NonRoomEvent::RoomName` event.
    async fn on_account_ignored_users(
        &mut self,
        _: Arc<Mutex<Room>>,
        _: Arc<Mutex<IgnoredUserListEvent>>,
    ) {
    }
    /// Fires when `AsyncClient` receives a `NonRoomEvent::RoomCanonicalAlias` event.
    async fn on_account_push_rules(&mut self, _: Arc<Mutex<Room>>, _: Arc<Mutex<PushRulesEvent>>) {}
    /// Fires when `AsyncClient` receives a `NonRoomEvent::RoomAliases` event.
    async fn on_account_data_fully_read(
        &mut self,
        _: Arc<Mutex<Room>>,
        _event: Arc<Mutex<FullyReadEvent>>,
    ) {
        
    }

    // `PresenceEvent` is a struct so there is only the one method
    /// Fires when `AsyncClient` receives a `NonRoomEvent::RoomAliases` event.
    async fn on_presence_event(&mut self, _: Arc<Mutex<Room>>, _event: Arc<Mutex<PresenceEvent>>) {}
}
