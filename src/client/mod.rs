use std::collections::HashMap;

use std::fmt;

use std::sync::Arc;

use anyhow::{Context, Result};

use matrix_sdk::{
    self,
    api::r0::{
        message::create_message_event,
    },
    events::{
        room::message::MessageEventContent,
    },
    identifiers::{RoomId, UserId},
    AsyncClient, AsyncClientConfig, Room, SyncSettings,
};
use tokio::sync::Mutex;
use url::Url;

pub mod client_loop;
pub mod event_stream;

#[derive(Clone)]
pub struct MatrixClient {
    pub inner: AsyncClient,
    homeserver: String,
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
            curr_sync: None,
        };

        Ok(client)
    }

    pub(crate) async fn login(
        &mut self,
        username: String,
        password: String,
    ) -> Result<HashMap<RoomId, Arc<Mutex<Room>>>> {
        let res = self.inner.login(username, password, None, None).await?;
        self.user = Some(res.user_id.clone());

        let _response = self
            .inner
            .sync(SyncSettings::new().full_state(true))
            .await?;

        Ok(self.inner.get_rooms().await)
    }

    pub(crate) async fn sync_forever(&mut self, settings: matrix_sdk::SyncSettings) -> Result<()> {
        self.inner
            .sync_forever(settings, move |_res| async {})
            .await;
        Ok(())
    }

    /// Sends a MessageEvent to the specified room.
    /// 
    /// # Arguments
    /// 
    /// * id - A valid RoomId otherwise sending will fail.
    /// * msg - `MessageEventContent`s is an enum that can handle all the types
    /// of messages eg. `Text`, `Audio`, `Video` ect.
    pub(crate) async fn send_message(
        &mut self,
        id: &RoomId,
        msg: MessageEventContent,
    ) -> Result<create_message_event::Response> {
        self.inner
            .room_send(&id, msg)
            .await
            .context("Message failed to send")
    }
}
