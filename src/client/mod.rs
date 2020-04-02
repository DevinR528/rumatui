use std::collections::HashMap;

use std::fmt;

use std::sync::{Arc, RwLock};

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
    AsyncClient, AsyncClientConfig, Room, SyncSettings,
};
use url::Url;
use tokio::sync::Mutex;

pub mod event_stream;

#[derive(Clone)]
pub struct MatrixClient {
    pub inner: AsyncClient,
    homeserver: String,
    pub current_room_id: Option<RoomId>,
    pub curr_sync: Option<String>,
    user: Option<UserId>,
}
unsafe impl Send for MatrixClient {}

impl fmt::Debug for MatrixClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MatrixClient")
            .field("user", &self.user)
            .finish()
    }
}

impl MatrixClient {
    pub fn new(homeserver: &str) -> Result<Self, failure::Error> {
        let _client_config = AsyncClientConfig::default();
        let homeserver_url = Url::parse(&homeserver)?;

        let client = Self {
            inner: AsyncClient::new(homeserver_url, None)?,
            homeserver: homeserver.into(),
            user: None,
            current_room_id: None,
            curr_sync: None,
        };

        Ok(client)
    }

    pub(crate) async fn current_room_id(&self) -> Option<RoomId> {
        self.inner.current_room_id().await
    }

    pub(crate) async fn login(
        &mut self,
        username: String,
        password: String,
    ) -> Result<HashMap<String, Arc<Mutex<Room>>>> {

        let res = self.inner.login(username, password, None, None).await?;
        self.user = Some(res.user_id.clone());

        let _response = self
            .inner
            .sync(SyncSettings::new().full_state(true))
            .await?;

        self.current_room_id = self.inner.current_room_id().await;
        Ok(self.inner.get_rooms().await)
    }

    pub(crate) async fn sync_forever(
        &mut self,
        settings: matrix_sdk::SyncSettings,
    ) -> Result<()> {

        self.inner.sync_forever(settings, move |_res| async { }).await;
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
}
