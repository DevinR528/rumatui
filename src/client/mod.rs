use std::{collections::HashMap, fmt, path::PathBuf, sync::Arc, time::Duration};

use matrix_sdk::{
    self,
    api::r0::{
        account::register::{self, RegistrationKind},
        membership::{forget_room, join_room_by_id, kick_user, leave_room},
        message::{create_message_event, get_message_events},
        read_marker::set_read_marker,
        receipt::create_receipt,
        session::login,
        typing::create_typing_event,
    },
    events::room::message::MessageEventContent,
    identifiers::{DeviceId, EventId, RoomId, UserId},
    Client, ClientConfig, JsonStore, RegistrationBuilder, Room, SyncSettings,
};
use tokio::sync::RwLock;
use url::Url;
use uuid::Uuid;

use crate::error::Result;

pub mod client_loop;
pub mod event_stream;
pub mod ruma_ext;

const SYNC_TIMEOUT: Duration = Duration::from_secs(30);

#[cfg(target_os = "linux")]
const RUMATUI_ID: &str = "rumatui command line client (LINUX)";

#[cfg(target_os = "windows")]
const RUMATUI_ID: &str = "rumatui command line client (WINDOWS)";

#[cfg(target_os = "macos")]
const RUMATUI_ID: &str = "rumatui command line client (MAC)";

#[derive(Clone)]
pub struct MatrixClient {
    pub inner: Client,
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
    pub fn new(homeserver: &str) -> Result<Self> {
        let homeserver = Url::parse(&homeserver)?;
        let path: Result<PathBuf> = dirs::home_dir()
            .ok_or(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "no home directory found",
            ))
            .map_err(Into::into);
        let mut path = path?;
        path.push(".rumatui");

        let store: Result<JsonStore> = JsonStore::open(path).map_err(Into::into);
        // reset the client with the state store with username as part of the store path
        let client_config = ClientConfig::default()
            // .proxy("http://localhost:8080")? // for mitmproxy
            // .disable_ssl_verification()
            .state_store(Box::new(store?));

        let inner: Result<Client> =
            Client::new_with_config(homeserver.clone(), client_config).map_err(Into::into);

        let client = Self {
            inner: inner?,
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
            .map_err(Into::into)
    }

    /// Log in to as the specified user.
    pub(crate) async fn login(
        &mut self,
        username: String,
        password: String,
    ) -> Result<(
        Arc<RwLock<HashMap<RoomId, Arc<RwLock<Room>>>>>,
        login::Response,
    )> {
        let res = self
            .inner
            .login(username, password, None, Some(RUMATUI_ID.to_string()))
            .await?;

        self.user = Some(res.user_id.clone());

        let _response = self
            .inner
            .sync(
                SyncSettings::default()
                    .timeout(SYNC_TIMEOUT)
                    .full_state(false),
            )
            .await?;

        self.next_batch = self.inner.sync_token().await;
        Ok((self.inner.joined_rooms(), res))
    }

    /// Create an account for the Matrix server used when starting the app.
    pub(crate) async fn register_user(
        &mut self,
        username: String,
        password: String,
        device_id: Option<DeviceId>,
    ) -> Result<register::Response> {
        let mut req = RegistrationBuilder::default();

        if let Some(device) = device_id {
            req.device_id(&device);
        }

        req.password(&password)
            .username(&username)
            .kind(RegistrationKind::User);

        self.inner.register_user(req).await.map_err(Into::into)
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
            .map_err(Into::into)
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

        match self.inner.room_messages(request).await {
            Ok(res) => {
                self.last_scroll
                    .insert(id.clone(), res.end.clone().unwrap());
                Ok(res)
            }
            err => err.map_err(Into::into),
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
            .map_err(Into::into)
    }

    /// Forgets the specified room.
    ///
    /// # Arguments
    ///
    /// * room_id - A valid RoomId otherwise sending will fail.
    pub(crate) async fn forget_room(&self, room_id: &RoomId) -> Result<forget_room::Response> {
        self.inner
            .forget_room_by_id(room_id)
            .await
            .map_err(Into::into)
    }

    /// Leaves the specified room.
    ///
    /// # Arguments
    ///
    /// * room_id - A valid RoomId otherwise sending will fail.
    pub(crate) async fn leave_room(&self, room_id: &RoomId) -> Result<leave_room::Response> {
        self.inner.leave_room(room_id).await.map_err(Into::into)
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
            .map_err(Into::into)
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
            .map_err(Into::into)
    }

    /// Send a request to notify the room the specific event has been seen.
    ///
    /// Returns a `create_typing_event::Response`, an empty response.
    ///
    /// # Arguments
    ///
    /// * room_id - The `RoomId` the user is typing in.
    ///
    /// * event_id - The `EventId` of the event the user has read to.
    pub async fn read_receipt(
        &self,
        room_id: &RoomId,
        event_id: &EventId,
    ) -> Result<create_receipt::Response> {
        self.inner
            .read_receipt(room_id, event_id)
            .await
            .map_err(Into::into)
    }

    /// Send a request to notify the room the user has seen up to `fully_read`.
    ///
    /// Returns a `set_read_marker::Response`, an empty response.
    ///
    /// # Arguments
    ///
    /// * room_id - The `RoomId` the user is typing in.
    ///
    /// * fully_read - The `EventId` of the event the user has read to.
    ///
    /// * read_receipt - The `EventId` to set the read receipt location at.
    pub async fn read_marker(
        &self,
        room_id: &RoomId,
        fully_read: &EventId,
        read_receipt: Option<&EventId>,
    ) -> Result<set_read_marker::Response> {
        self.inner
            .read_marker(room_id, fully_read, read_receipt)
            .await
            .map_err(Into::into)
    }
}
