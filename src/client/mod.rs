use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use matrix_sdk::{
    self,
    api::r0::membership::{
        forget_room, join_room_by_id, kick_user, leave_room,
    },
    // api::r0::filter::{LazyLoadOptions, RoomEventFilter},
    api::r0::message::{create_message_event, get_message_events},
    api::r0::session::login,
    api::r0::typing::create_typing_event,
    api::r0::receipt::create_receipt,
    events::room::message::MessageEventContent,
    identifiers::{EventId, RoomId, UserId},
    AsyncClient,
    AsyncClientConfig,
    JsonStore,
    Room,
    SyncSettings,
};
use tokio::sync::RwLock;
use url::Url;
use uuid::Uuid;

pub mod client_loop;
pub mod event_stream;

const SYNC_TIMEOUT: Duration = Duration::from_secs(30);

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
        let mut path = dirs::home_dir().ok_or(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "no home directory found",
        ))?;
        path.push(".rumatui");
        // reset the client with the state store with username as part of the store path
        let client_config = AsyncClientConfig::default()
            .proxy("http://localhost:8080")? // for mitmproxy
            .disable_ssl_verification()
            .state_store(Box::new(JsonStore::open(path)?));

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

    /// Joins the specified room.
    ///
    /// # Arguments
    ///
    /// * room_id - A valid RoomId otherwise sending will fail.
    pub(crate) async fn store_room_state(&self, room_id: &RoomId) -> Result<()> {
        self.inner
            .store_room_state(room_id)
            .await
            .context(format!("Storing state of room {} failed", room_id))
    }

    /// Log in to as the specified user.
    pub(crate) async fn login(
        &mut self,
        username: String,
        password: String,
    ) -> Result<(HashMap<RoomId, Arc<RwLock<Room>>>, login::Response)> {
        let res = self.inner.login(username, password, None, None).await?;
        self.user = Some(res.user_id.clone());

        // if we can't sync with the "Db" then we must sync with the server
        if !self.inner.sync_with_state_store().await? {
            let _response = self
                .inner
                .sync(
                    SyncSettings::default()
                        .timeout(SYNC_TIMEOUT)
                        .full_state(false),
                )
                .await?;
        }

        self.next_batch = self.inner.sync_token().await;
        Ok((self.inner.get_rooms().await, res))
    }

    /// Manually sync state, provides a default sync token if None is given.
    ///
    /// This can be useful when joining a room, we need the state from before our sync_token.
    pub(crate) async fn sync(&mut self, setting: Option<SyncSettings>) -> Result<()> {
        let settings = setting.unwrap_or(
            SyncSettings::default()
                .timeout(SYNC_TIMEOUT)
                .full_state(false),
        );
        let _response = self.inner.sync(settings).await?;

        self.next_batch = self.inner.sync_token().await;
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
        &self,
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

    /// Joins the specified room.
    ///
    /// # Arguments
    ///
    /// * room_id - A valid RoomId otherwise sending will fail.
    pub(crate) async fn join_room_by_id(
        &self,
        room_id: &RoomId,
    ) -> Result<join_room_by_id::Response> {
        self.inner
            .join_room_by_id(room_id)
            .await
            .context(format!("Joining room {} failed", room_id))
    }

    /// Forgets the specified room.
    ///
    /// # Arguments
    ///
    /// * room_id - A valid RoomId otherwise sending will fail.
    pub(crate) async fn forget_room_by_id(
        &self,
        room_id: &RoomId,
    ) -> Result<forget_room::Response> {
        self.inner
            .forget_room_by_id(room_id)
            .await
            .context(format!("Forgetting room {} failed", room_id))
    }

    /// Leaves the specified room.
    ///
    /// # Arguments
    ///
    /// * room_id - A valid RoomId otherwise sending will fail.
    pub(crate) async fn leave_room(&self, room_id: &RoomId) -> Result<leave_room::Response> {
        self.inner
            .leave_room(room_id)
            .await
            .context(format!("Leaving room {} failed", room_id))
    }

    /// Kicks the specified user from the room.
    ///
    /// # Arguments
    ///
    /// * room_id - The `RoomId` of the room the user should be kicked out of.
    ///
    /// * user_id - The `UserId` of the user that should be kicked out of the room.
    ///
    /// * reason - Optional reason why the room member is being kicked out.
    pub(crate) async fn kick_user(
        &self,
        room_id: &RoomId,
        user_id: &UserId,
        reason: Option<String>,
    ) -> Result<kick_user::Response> {
        self.inner
            .kick_user(room_id, user_id, reason)
            .await
            .context(format!("Leaving room {} failed", room_id))
    }

    /// Send a request to notify the room of a user typing.
    ///
    /// Returns a `create_typing_event::Response`, an empty response.
    ///
    /// # Arguments
    ///
    /// * room_id - The `RoomId` the user is typing in.
    ///
    /// * user_id - The `UserId` of the user that is typing.
    ///
    /// * typing - Whether the user is typing, if false `timeout` is not needed.
    ///
    /// * timeout - Length of time in milliseconds to mark user is typing.
    pub async fn typing_notice(
        &self,
        room_id: &RoomId,
        user_id: &UserId,
        typing: bool,
        timeout: Option<Duration>,
    ) -> Result<create_typing_event::Response> {
        self.inner
            .typing_notice(room_id, user_id, typing, timeout)
            .await
            .context(format!("failed to send typing notification to {}", room_id))
    }

    /// Send a request to notify the room of a user typing.
    ///
    /// Returns a `create_typing_event::Response`, an empty response.
    ///
    /// # Arguments
    ///
    /// * room_id - The `RoomId` the user is typing in.
    ///
    /// * event_id - The `UserId` of the user that is typing.
    ///
    /// * typing - Whether the user is typing, if false `timeout` is not needed.
    ///
    /// * timeout - Length of time in milliseconds to mark user is typing.
    pub async fn read_receipt(
        &self,
        room_id: &RoomId,
        event_id: &EventId,
    ) -> Result<create_receipt::Response> {
        self.inner
            .read_receipt(room_id, event_id)
            .await
            .context(format!("failed to send read_receipt to {}", room_id))
    }
}
