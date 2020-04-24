use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;
use std::path::PathBuf;

use anyhow::{Context, Result};
use matrix_sdk::{
    self,
    // api::r0::filter::{LazyLoadOptions, RoomEventFilter},
    api::r0::message::create_message_event,
    api::r0::message::get_message_events,
    api::r0::session::login,
    events::room::message::MessageEventContent,
    identifiers::{RoomId, UserId},
    AsyncClient,
    AsyncClientConfig,
    JsonStore,
    Room,
    StateStore,
    SyncSettings,
};
use tokio::sync::RwLock;
use url::Url;
use uuid::Uuid;

pub mod client_loop;
pub mod event_stream;

const SYNC_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone)]
pub struct MatrixClient {
    /// TODO once matrix-sdk `StateStore` is impled make this work
    pub inner: AsyncClient,
    homeserver: Url,
    user: Option<UserId>,
    settings: SyncSettings,
    next_batch: Option<String>,
    last_scroll: HashMap<RoomId, String>,
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
        let homeserver = Url::parse(&homeserver)?;
        let mut path = dirs::home_dir().ok_or(std::io::Error::new(std::io::ErrorKind::NotFound, "no home directory found"))?;
        path.push(".rumatui");
        // reset the client with the state store with username as part of the store path
        let client_config = AsyncClientConfig::default().state_store(Box::new(JsonStore::open(path)?));

        let client = Self {
            inner: AsyncClient::new_with_config(homeserver.clone(), None, client_config)?,
            homeserver,
            user: None,
            settings: SyncSettings::default(),
            next_batch: None,
            last_scroll: HashMap::new(),
        };

        Ok(client)
    }

    pub fn sync_token(&self) -> Option<String> {
        self.next_batch.clone()
    }

    pub(crate) async fn login(
        &mut self,
        username: String,
        password: String,
    ) -> Result<(HashMap<RoomId, Arc<RwLock<Room>>>, login::Response)> {
        self.inner.append_state_store_path(&PathBuf::from(format!("{}", username))).await;

        let res = self.inner.login(username, password, None, None).await?;
        self.user = Some(res.user_id.clone());

        let _response = self
            .inner
            .sync(SyncSettings::default().timeout(SYNC_TIMEOUT))
            .await?;
        self.next_batch = self.inner.sync_token().await;

        Ok((self.inner.get_rooms().await, res))
    }

    pub(crate) async fn sync(&mut self) -> Result<()> {
        let tkn = self.sync_token().unwrap();

        self.settings = SyncSettings::new().token(tkn).timeout(SYNC_TIMEOUT);
        self.inner
            .sync(self.settings.to_owned())
            .await
            .map(|_res| ())
            .map_err(|e| anyhow::Error::from(e))
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
        uuid: Uuid,
    ) -> Result<create_message_event::Response> {
        self.inner
            .room_send(&id, msg, Some(uuid))
            .await
            .context("Message failed to send")
    }

    /// Gets the `RoomEvent`s backwards in time, when user scrolls up.
    ///
    /// This uses the current sync token to look backwards from that point.
    ///
    /// # Arguments
    ///
    /// * id - A valid RoomId otherwise sending will fail.
    ///
    pub(crate) async fn get_messages(
        &mut self,
        id: &RoomId,
    ) -> Result<get_message_events::Response> {
        let from = if let Some(scroll) = self.last_scroll.get(id) {
            scroll.clone()
        } else {
            self.next_batch.as_ref().unwrap().clone()
        };
        let request = get_message_events::Request {
            room_id: id.clone(),
            from,
            to: None,
            dir: get_message_events::Direction::Backward,
            limit: js_int::UInt::new(30),
            filter: None,
            // filter: Some(RoomEventFilter {
            //     lazy_load_options: LazyLoadOptions::Enabled { include_redundant_members: false, },
            //     .. Default::default()
            // }),
        };

        match self
            .inner
            .room_messages(request)
            .await
            .map_err(|e| anyhow::Error::from(e))
        {
            Ok(res) => {
                self.last_scroll.insert(id.clone(), res.end.clone());
                Ok(res)
            }
            err => err,
        }
    }
}
