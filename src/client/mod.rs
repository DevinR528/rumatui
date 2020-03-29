use std::collections::HashMap;
use std::convert::TryFrom;
use std::fmt;
use std::io::{self, Write};
use std::sync::{Arc, RwLock};

use failure::Fail;
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
    identifiers::{},
    ruma_traits::{Endpoint, Outgoing},
    AsyncClient, AsyncClientConfig, Room, SyncSettings,
};

use url::Url;

#[derive(Clone, Debug)]
pub struct RoomInfo {
    pub name: Option<String>,
    pub alias: Option<RoomAliasId>,
    pub user: UserId,
}
impl RoomInfo {
    pub(crate) fn from_name(user: UserId, name: &str) -> Self {
        Self {
            name: Some(name.to_string()),
            user,
            alias: None,
        }
    }
    pub(crate) fn from_alias(user: UserId, alias: RoomAliasId) -> Self {
        Self {
            name: None,
            user,
            alias: Some(alias),
        }
    }
}
#[derive(Clone)]
pub struct MatrixClient {
    inner: AsyncClient,
    homeserver: String,
    room_ids: Vec<RoomId>,
    pub rooms: HashMap<RoomId, RoomInfo>,
    curr_sync: Option<String>,
    user: String,
}

impl fmt::Debug for MatrixClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MatrixClient")
            .field("user", &self.user)
            .finish()
    }
}

impl MatrixClient {
    pub fn new(homeserver: &str) -> Result<Self, failure::Error> {
        let client_config = AsyncClientConfig::default();
        let homeserver_url = Url::parse(&homeserver)?;

        let mut client = Self {
            inner: AsyncClient::new(homeserver_url, None)?,
            homeserver: homeserver.into(),
            user: String::default(),
            room_ids: Vec::default(),
            rooms: HashMap::new(),
            curr_sync: None,
        };

        Ok(client)
    }

    pub(crate) async fn login(
        &mut self,
        username: String,
        password: String,
    ) -> Result<()> {
        self.inner.add_event_callback(room_create);
        self.inner.add_event_callback(room_name);

        let res = self.inner.login(username, password, None).await?;

        let response = self
            .inner
            .sync(SyncSettings::new().full_state(true))
            .await?;

        self.process_response(response).await?;

        Ok(())
    }

    pub(crate) async fn send_message(
        &self,
        client: &mut AsyncClient,
        id: &str,
        msg: MessageEventContent,
    ) -> Result<create_message_event::Response> {
        client.room_send(&id, msg).await.context("Message failed to send")
    }

    async fn process_response(
        &mut self,
        response: sync_events::IncomingResponse,
    ) -> Result<()> {
        // next batch token keeps track of sync up to this point
        self.curr_sync = Some(response.next_batch.clone());
        // joined rooms, left or banned (leave), invited (invite)

        self.room_ids = response.rooms.join.keys().cloned().collect::<Vec<RoomId>>();
        // let _to_device = response.to_device;
        // let key_map = response.device_one_time_keys_count;

        // map the room_ids to room names and aliases
        for room in response.rooms.join.values() {
            for ev in room
                .state
                .events
                .iter()
                .flat_map(|res| res.clone().into_result().ok())
            {
                match ev {
                    StateEvent::RoomName(e) => {
                        if let Some(name) = e.content.name() {
                            self.map_rooms(name, &e.sender).await?;
                        }
                    }
                    StateEvent::RoomCanonicalAlias(e) => {
                        if let Some(alias) = e.content.alias {
                            self.map_rooms(&alias.to_string(), &e.sender).await?;
                        }
                    }
                    StateEvent::RoomAliases(e) => {
                        if let Some(alias) = e.content.aliases.first() {
                            self.map_rooms(&alias.to_string(), &e.sender).await?;
                        }
                    }
                    _ => {}
                }
            }
        }
        // map using timeline incase we missed any in the state
        for room in response.rooms.join.values() {
            for ev in room
                .timeline
                .events
                .iter()
                .flat_map(|res| res.clone().into_result().ok())
            {
                match ev {
                    RoomEvent::RoomName(e) => {
                        if let Some(name) = e.content.name() {
                            self.map_rooms(name, &e.sender).await?;
                        }
                    }
                    RoomEvent::RoomCanonicalAlias(e) => {
                        if let Some(alias) = e.content.alias {
                            self.map_rooms(&alias.to_string(), &e.sender).await?;
                        }
                    }
                    RoomEvent::RoomAliases(e) => {
                        if let Some(alias) = e.content.aliases.first() {
                            self.map_rooms(&alias.to_string(), &e.sender).await?;
                        }
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }

    /// After finding the state event that contains the name or alias for the room we then
    /// have to make a search room request to obtain the room_id connected to a m.room.name
    pub(crate) async fn map_rooms(
        &mut self,
        name: &str,
        user: &UserId,
    ) -> Result<()> {
        let data = get_public_rooms_filtered::Request {
            server: None,
            limit: None,
            since: None,
            filter: Some(get_public_rooms_filtered::Filter {
                generic_search_term: Some(name.to_string()),
            }),
        };
        let res = self.inner.send(data).await?;

        let rooms = &res.chunk;
        for room in rooms {
            self.rooms.entry(room.room_id.clone()).or_insert_with(|| {
                if let Some(name) = &room.name {
                    RoomInfo::from_name(user.clone(), name)
                } else if let Some(alias) = &room.canonical_alias {
                    RoomInfo::from_alias(
                        user.clone(),
                        RoomAliasId::try_from(alias.as_str()).unwrap(),
                    )
                } else if let Some(alias) = &room.aliases {
                    if let Some(alias) = alias.first() {
                        RoomInfo::from_alias(user.clone(), alias.clone())
                    } else {
                        panic!("no aliases in AliasEvent")
                    }
                } else {
                    panic!("No canonical alias or room name")
                }
            });
        }
        Ok(())
    }

    pub(crate) async fn ping_room(
        &mut self,
        room_id: &RoomId,
    ) -> Result<search_events::IncomingResponse> {
        let data = search_events::Request {
            next_batch: None,
            search_categories: Categories {
                room_events: Some(Criteria {
                    event_context: None,
                    filter: Some(RoomEventFilter {
                        not_types: Vec::default(),
                        not_rooms: Vec::default(),
                        limit: None,
                        rooms: Some(vec![room_id.clone()]),
                        types: Some(vec![
                            "m.room.canonical_alias".into(),
                            "m.room.aliases".into(),
                            "m.room.name".into(),
                        ]),
                        ..Default::default()
                    }),
                    groupings: None,
                    include_state: None,
                    keys: Vec::default(),
                    order_by: None,
                    search_term: String::from("name"),
                }),
            },
        };
        let response = self.inner.send(data).await?;
        println!("ROOM {:?}", response);
        Ok(response)
    }
}

pub(crate) async fn room_member(room: Arc<RwLock<Room>>, event: Arc<EventResult<RoomEvent>>) {
    let room = room.read().unwrap();
    let event = if let EventResult::Ok(event) = &*event {
        event
    } else {
        return;
    };
    if let RoomEvent::RoomMember(MemberEvent {
        content,
        room_id,
        sender,
        ..
    }) = event
    {
        println!(
            "{}: {:?}",
            content
                .displayname
                .as_ref()
                .unwrap_or(&sender.to_string())
                .trim(),
            room_id
        );
    }
}

pub(crate) async fn room_alias(room: Arc<RwLock<Room>>, event: Arc<EventResult<RoomEvent>>) {
    let room = room.read().unwrap();
    let event = if let EventResult::Ok(event) = &*event {
        event
    } else {
        return;
    };
    if let RoomEvent::RoomAliases(AliasesEvent {
        content,
        room_id,
        sender,
        ..
    }) = event
    {
        println!(
            "ALIAS {:?}: {:?}",
            room_id,
            content
                .aliases
                .iter()
                .map(|al| al.alias())
                .collect::<Vec<_>>(),
        );
    }
}

pub(crate) async fn room_create(room: Arc<RwLock<Room>>, event: Arc<EventResult<RoomEvent>>) {
    let room = room.read().unwrap();
    let event = if let EventResult::Ok(event) = &*event {
        event
    } else {
        return;
    };
    if let RoomEvent::RoomCreate(CreateEvent {
        content,
        room_id,
        sender,
        ..
    }) = event
    {
        println!("ALIAS {:?}: {:?}", room_id, content.room_version,);
    }
}

pub(crate) async fn canonical_room_name(
    room: Arc<RwLock<Room>>,
    event: Arc<EventResult<RoomEvent>>,
) {
    let room = room.read().unwrap();
    let event = if let EventResult::Ok(event) = &*event {
        event
    } else {
        return;
    };
    if let RoomEvent::RoomCanonicalAlias(CanonicalAliasEvent {
        content,
        room_id,
        sender,
        ..
    }) = event
    {
        println!(
            "ALIAS {:?}: {:?}",
            room_id,
            content.alias.as_ref().map(|al| al.alias()),
        );
    }
}

pub(crate) async fn room_name(room: Arc<RwLock<Room>>, event: Arc<EventResult<RoomEvent>>) {
    let room = room.read().unwrap();
    let event = if let EventResult::Ok(event) = &*event {
        event
    } else {
        return;
    };
    if let RoomEvent::RoomName(NameEvent {
        content,
        prev_content,
        room_id,
        sender,
        ..
    }) = event
    {
        let user = room.members.get(&sender.to_string()).unwrap();
        println!(
            "{:?} = {:?}: {:?} {:?}",
            user.display_name,
            content.name(),
            room_id,
            prev_content.as_ref().map(|c| c.name()),
        );
    }
}

pub(crate) fn read_test_info() -> HashMap<String, String> {
    const KEYS: &[&str] = &["url", "user", "password", "community", "room"];
    std::fs::read_to_string("./info.txt")
        .unwrap()
        .split("\n")
        .zip(KEYS)
        .map(|(a, b)| (b.to_string(), a.to_string()))
        .collect()
}
