use matrix_sdk::{
    self,
    api::r0::{
        directory::get_public_rooms_filtered,
        filter::RoomEventFilter,
        message::create_message_event,
        search::search_events::{self, Categories, Criteria},
        sync::sync_events,
    },
    events::{
        collections::all::{RoomEvent, StateEvent},
        room::aliases::AliasesEvent,
        room::canonical_alias::CanonicalAliasEvent,
        room::create::CreateEvent,
        room::member::{MemberEvent, MembershipState},
        room::message::{MessageEvent, MessageEventContent, TextMessageEventContent},
        room::name::{NameEvent, NameEventContent},
        EventResult, EventType,
    },
    identifiers::{UserId, RoomId, RoomAliasId},
    ruma_traits::{Endpoint, Outgoing},
    AsyncClient, AsyncClientConfig, Room, SyncSettings, EventEmitter,
};
pub struct EventStream {

}

impl EventEmitter for EventStream {

}
