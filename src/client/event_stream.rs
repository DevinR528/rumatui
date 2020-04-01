use std::sync::Arc;
use std::ops::Deref;

use matrix_sdk::{
    self,
    events::{
        collections::all::{RoomEvent, StateEvent},
        collections::only::Event as NonRoomEvent,
        presence::PresenceEvent,
        room::message::{MessageEvent, MessageEventContent, TextMessageEventContent}
    },
    identifiers::{RoomId, RoomAliasId, UserId},
    EventEmitter, Room,
};

use tokio::sync::Mutex;
use tokio::sync::mpsc;




pub enum StateResult {
    Message(String, String),
    Err,
}
unsafe impl Send for StateResult {}

pub struct EventStream {
    send: mpsc::Sender<StateResult>
}
unsafe impl Send for EventStream {}

impl EventStream {
    pub(crate) fn new() -> (Self, mpsc::Receiver<StateResult>) {
        let (send, recv) = mpsc::channel(1024);

        (Self { send, }, recv) 
    }
}

#[async_trait::async_trait]
impl EventEmitter for EventStream {
     /// Fires when `AsyncClient` receives a `RoomEvent::RoomMember` event.
     async fn on_room_member(&mut self, room: Arc<Mutex<Room>>, event: Arc<Mutex<RoomEvent>>) {}
     /// Fires when `AsyncClient` receives a `RoomEvent::RoomName` event.
     async fn on_room_name(&mut self, room: Arc<Mutex<Room>>, event: Arc<Mutex<RoomEvent>>) {}
     /// Fires when `AsyncClient` receives a `RoomEvent::RoomCanonicalAlias` event.
     async fn on_room_canonical_alias(&mut self, room: Arc<Mutex<Room>>, event: Arc<Mutex<RoomEvent>>) {}
     /// Fires when `AsyncClient` receives a `RoomEvent::RoomAliases` event.
     async fn on_room_aliases(&mut self, room: Arc<Mutex<Room>>, event: Arc<Mutex<RoomEvent>>) {}
     /// Fires when `AsyncClient` receives a `RoomEvent::RoomAvatar` event.
     async fn on_room_avatar(&mut self, room: Arc<Mutex<Room>>, event: Arc<Mutex<RoomEvent>>) {}
     /// Fires when `AsyncClient` receives a `RoomEvent::RoomMessage` event.
     async fn on_room_message(&mut self, room: Arc<Mutex<Room>>, event: Arc<Mutex<RoomEvent>>) {
        let r = room.lock().await;
        let Room { room_id, room_name, members, .. } = r.deref();
        let ev = event.lock().await;
        let ev = ev.deref();

        if let RoomEvent::RoomMessage(MessageEvent {
            content,
            sender,
            ..
        }) = ev {
            let name = if let Some(mem) = members.get(&sender.to_string()) {
                mem.name.clone()
            } else {
                sender.localpart().into()
            };
            match content {
                MessageEventContent::Text(TextMessageEventContent { body: msg_body, .. }) => {
                    if let Err(e) = self.send.send(StateResult::Message(name, msg_body.clone())).await {
                        panic!("{}", e)
                    }
                }
                _ => {},
            }
        }
     }
     /// Fires when `AsyncClient` receives a `RoomEvent::RoomMessageFeedback` event.
     async fn on_room_message_feedback(&mut self, room: Arc<Mutex<Room>>, event: Arc<Mutex<RoomEvent>>) {}
     /// Fires when `AsyncClient` receives a `RoomEvent::RoomRedaction` event.
     async fn on_room_redaction(&mut self, room: Arc<Mutex<Room>>, event: Arc<Mutex<RoomEvent>>) {}
     /// Fires when `AsyncClient` receives a `RoomEvent::RoomPowerLevels` event.
     async fn on_room_power_levels(&mut self, room: Arc<Mutex<Room>>, event: Arc<Mutex<RoomEvent>>) {}
 
     // `RoomEvent`s from `IncomingState`
     /// Fires when `AsyncClient` receives a `StateEvent::RoomMember` event.
     async fn on_state_member(&mut self, room: Arc<Mutex<Room>>, event: Arc<Mutex<StateEvent>>) {}
     /// Fires when `AsyncClient` receives a `StateEvent::RoomName` event.
     async fn on_state_name(&mut self, room: Arc<Mutex<Room>>, event: Arc<Mutex<StateEvent>>) {}
     /// Fires when `AsyncClient` receives a `StateEvent::RoomCanonicalAlias` event.
     async fn on_state_canonical_alias(&mut self, room: Arc<Mutex<Room>>, event: Arc<Mutex<StateEvent>>) {}
     /// Fires when `AsyncClient` receives a `StateEvent::RoomAliases` event.
     async fn on_state_aliases(&mut self, room: Arc<Mutex<Room>>, event: Arc<Mutex<StateEvent>>) {}
     /// Fires when `AsyncClient` receives a `StateEvent::RoomAvatar` event.
     async fn on_state_avatar(&mut self, room: Arc<Mutex<Room>>, event: Arc<Mutex<StateEvent>>) {}
     /// Fires when `AsyncClient` receives a `StateEvent::RoomPowerLevels` event.
     async fn on_state_power_levels(&mut self, room: Arc<Mutex<Room>>, event: Arc<Mutex<StateEvent>>) {}
     /// Fires when `AsyncClient` receives a `StateEvent::RoomJoinRules` event.
     async fn on_state_join_rules(&mut self, room: Arc<Mutex<Room>>, event: Arc<Mutex<StateEvent>>) {}
 
     // `NonRoomEvent` (this is a type alias from ruma_events) from `IncomingAccountData`
     /// Fires when `AsyncClient` receives a `NonRoomEvent::RoomMember` event.
     async fn on_account_presence(&mut self, room: Arc<Mutex<Room>>, event: Arc<Mutex<NonRoomEvent>>) {}
     /// Fires when `AsyncClient` receives a `NonRoomEvent::RoomName` event.
     async fn on_account_ignored_users(&mut self, room: Arc<Mutex<Room>>, event: Arc<Mutex<NonRoomEvent>>) {}
     /// Fires when `AsyncClient` receives a `NonRoomEvent::RoomCanonicalAlias` event.
     async fn on_account_push_rules(&mut self, room: Arc<Mutex<Room>>, event: Arc<Mutex<NonRoomEvent>>) {}
     /// Fires when `AsyncClient` receives a `NonRoomEvent::RoomAliases` event.
     async fn on_account_data_fully_read(&mut self, room: Arc<Mutex<Room>>, event: Arc<Mutex<NonRoomEvent>>) {}
 
     // `PresenceEvent` is a struct so there is only the one method
     /// Fires when `AsyncClient` receives a `NonRoomEvent::RoomAliases` event.
     async fn on_presence_event(&mut self, room: Arc<Mutex<Room>>, event: Arc<Mutex<PresenceEvent>>) {}
}
