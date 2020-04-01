use std::fmt;
use std::thread;
use std::time::Duration;
use std::ops::{Deref, DerefMut};
use std::marker::PhantomData;
use std::collections::HashMap;
use std::sync::{atomic::AtomicBool, Arc, RwLock};

use anyhow::{Result, Context};
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
use tokio::task::JoinHandle;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Sender, Receiver};
use tokio::runtime::Handle;
use tokio::sync::Mutex;

pub enum StateResult {
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

impl EventEmitter for EventStream {
    
}
