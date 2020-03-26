use std::collections::HashMap;
use std::convert::TryFrom;
use std::fmt;
use std::io::{self, Write};
use std::sync::{Arc, RwLock};

use matrix_sdk::{
    self,
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
    AsyncClient, AsyncClientConfig, Room, SyncSettings,
};
use ruma_api::{Endpoint, Outgoing};
use ruma_client_api::r0::{
    filter::RoomEventFilter,
    message::create_message_event,
    search::search_events::{self, Categories, Criteria},
    sync::sync_events,
};
use ruma_identifiers::RoomId;
use url::Url;

#[derive(Clone)]
pub struct MatrixClient {
    inner: AsyncClient,
    homeserver: String,
    room_ids: Vec<RoomId>,
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
    pub async fn new(homeserver: &str) -> Result<Self, failure::Error> {
        let client_config = AsyncClientConfig::default();
        let homeserver_url = Url::parse(&homeserver)?;

        let mut client = Self {
            inner: AsyncClient::new(homeserver_url, None)?,
            homeserver: homeserver.into(),
            user: String::default(),
            room_ids: Vec::default(),
            curr_sync: None,
        };

        Ok(client)
    }

    pub(crate) async fn login(
        &mut self,
        username: String,
        password: String,
    ) -> Result<(), matrix_sdk::Error> {
        self.inner.add_event_callback(room_create);
        self.inner.add_event_callback(room_name);

        let res = self.inner.login(username, password, None).await?;
        println!("{:?}", res);
        println!();
        
        let response = self.inner.sync(SyncSettings::new()).await?;

        self.process_response(response).await?;
        let room_ids = self.room_ids.clone();
        for room_id in room_ids.iter() {
            self.ping_room(room_id).await?;
        }
        panic!();

        Ok(())
    }

    pub(crate) async fn send_message(
        &self,
        client: &mut AsyncClient,
        id: &str,
        msg: MessageEventContent,
    ) -> Result<create_message_event::Response, matrix_sdk::Error> {
        client.room_send(&id, msg).await
    }

    async fn process_response(
        &mut self,
        response: sync_events::IncomingResponse,
    ) -> Result<(), matrix_sdk::Error> {
        // next batch token keeps track of sync up to this point
        self.curr_sync = Some(response.next_batch.clone());
        // joined rooms, left or banned (leave), invited (invite)
        self.room_ids = response.rooms.join.keys().cloned().collect::<Vec<RoomId>>();
        println!("ROOM IDS LEN {}", self.room_ids.len());

        // let joined = response
        //     .rooms
        //     .join
        //     .iter()
        //     .map(|(id, room)| id)
        //     .collect::<Vec<RoomId>>();
        // vec of events
        // let names = response
        //     .presence
        //     .events
        //     .iter()
        //     .flat_map(|res| res.clone().into_result().ok())
        //     .flat_map(|ev| ev.content.displayname)
        //     .collect::<Vec<_>>();

        // let _to_device = response.to_device;
        // let key_map = response.device_one_time_keys_count;
        // println!("{:?}", next_tkn);
        // println!(
        //     "{:?}",
        //     room_ids
        //         .iter()
        //         .map(|id| format!("!{}:{}", id.hostname(), id.localpart()))
        //         .collect::<Vec<_>>()
        // );
        // println!("{:?}", names);

        if let Some(id) = self.room_ids
            .iter()
            .map(|id| format!("!{}:{}", id.localpart(), id.hostname(),))
            .find(|id| id.starts_with("!TFAjFxMDQqAluONEko"))
        {
            // let res = send_message(&mut client, &id, MessageEventContent::Text(TextMessageEventContent {
            //     body: "From RumaTui".into(),
            //     format: None,
            //     formatted_body: None,
            //     relates_to: None,
            // })).await?;
            // println!("{:?}", res);
            let res = self.ping_room(&RoomId::try_from(id.as_str()).unwrap()).await?;
        };

        Ok(())
    }

    pub(crate) async fn ping_room(
        &mut self,
        room_id: &RoomId,
    ) -> Result<search_events::IncomingResponse, matrix_sdk::Error> {
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
                        types: Some(vec!["m.room.canonical_alias".into(), "m.room.aliases".into(), "m.room.name".into()]),
                        .. Default::default()
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
