use std::collections::HashMap;
use std::fmt;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;

use anyhow::Result;
use matrix_sdk::Room;
use tokio::runtime::Handle;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Sender;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::client::event_stream::EventStream;
use crate::client::MatrixClient;
use matrix_sdk::api::r0::membership::{join_room_by_id, leave_room};
use matrix_sdk::api::r0::message::{create_message_event, get_message_events};
use matrix_sdk::api::r0::receipt::create_receipt;
use matrix_sdk::api::r0::session::login;
use matrix_sdk::api::r0::typing::create_typing_event;
use matrix_sdk::events::room::message::MessageEventContent;
use matrix_sdk::identifiers::{EventId, RoomId, UserId};

/// Requests sent from the UI portion of the app.
///
/// Each request is sent in response to some user input.
pub enum UserRequest {
    Login(String, String),
    SendMessage(RoomId, MessageEventContent, Uuid),
    RoomMsgs(RoomId),
    AcceptInvite(RoomId),
    DeclineInvite(RoomId),
    LeaveRoom(RoomId),
    Typing(RoomId, UserId),
    ReadReceipt(RoomId, EventId),
    Quit,
}
unsafe impl Send for UserRequest {}

impl fmt::Debug for UserRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Login(name, _) => write!(f, "failed login for {}", name),
            Self::SendMessage(id, _, _) => write!(f, "failed sending message for {}", id),
            Self::RoomMsgs(id) => write!(f, "failed to get room messages for {}", id),
            Self::AcceptInvite(id) => write!(f, "failed to join {}", id),
            Self::DeclineInvite(id) => write!(f, "failed to decline {}", id),
            Self::LeaveRoom(id) => write!(f, "failed to leave {}", id),
            Self::ReadReceipt(room, event) => {
                write!(f, "failed to send read_receipt for {} in {}", event, room)
            }
            Self::Typing(id, user) => {
                write!(f, "failed to send typing event for {} in {}", user, id)
            }
            Self::Quit => write!(f, "quitting filed"),
        }
    }
}

/// Either a `UserRequest` succeeds or fails with the given result.
pub enum RequestResult {
    Login(
        Result<(
            Arc<RwLock<HashMap<RoomId, Arc<RwLock<Room>>>>>,
            login::Response,
        )>,
    ),
    SendMessage(Result<create_message_event::Response>),
    RoomMsgs(Result<(get_message_events::Response, Arc<RwLock<Room>>)>),
    AcceptInvite(Result<join_room_by_id::Response>),
    DeclineInvite(Result<leave_room::Response>, RoomId),
    LeaveRoom(Result<leave_room::Response>, RoomId),
    Typing(Result<create_typing_event::Response>),
    ReadReceipt(Result<create_receipt::Response>),
    Error(anyhow::Error),
}

unsafe impl Send for RequestResult {}

/// The main task event loop.
///
/// `MatrixEventHandle` controls the `sync_forever` and user request loop.
pub struct MatrixEventHandle {
    cli_jobs: JoinHandle<Result<()>>,
    sync_jobs: JoinHandle<Result<()>>,
    start_sync: Arc<AtomicBool>,
    quit_flag: Arc<AtomicBool>,
}
unsafe impl Send for MatrixEventHandle {}

impl MatrixEventHandle {
    pub async fn new(
        stream: EventStream,
        mut to_app: Sender<RequestResult>,
        exec_hndl: Handle,
        homeserver: &str,
    ) -> (Self, Sender<UserRequest>) {
        let (app_sender, mut recv) = mpsc::channel(1024);

        let mut client = MatrixClient::new(homeserver).unwrap();
        client.inner.add_event_emitter(Box::new(stream)).await;

        let cli = client.inner.clone();
        // when the ui loop logs in start_sync releases and starts `sync_forever`
        let start_sync = Arc::from(AtomicBool::from(false));
        let quit_flag = Arc::from(AtomicBool::from(false));

        let is_sync = Arc::clone(&start_sync);
        let quitting = Arc::clone(&quit_flag);
        // this loop uses the above `AtomicBool` to signal shutdown.
        let sync_jobs = exec_hndl.spawn(async move {
            while !is_sync.load(Ordering::SeqCst) {
                if quitting.load(Ordering::SeqCst) {
                    return Ok(());
                }

                std::sync::atomic::spin_loop_hint();
            }

            if quitting.load(Ordering::SeqCst) {
                return Ok(());
            }
            let set = matrix_sdk::SyncSettings::default();
            cli.sync_forever(set.clone(), |_| async {}).await;
            Ok(())
        });

        // this loop is shutdown with a channel message
        let cli_jobs = exec_hndl.spawn(async move {
            loop {
                let input = recv.recv().await;
                if input.is_none() {
                    return Ok(());
                }

                match input.unwrap() {
                    UserRequest::Quit => return Ok(()),
                    UserRequest::Login(u, p) => {
                        let res = client.login(u, p).await;
                        if let Err(e) = to_app.send(RequestResult::Login(res)).await {
                            panic!("client event handler crashed {}", e)
                        }
                    }
                    UserRequest::SendMessage(room, msg, uuid) => {
                        let res = client.send_message(&room, msg, uuid).await;
                        if let Err(e) = to_app.send(RequestResult::SendMessage(res)).await {
                            panic!("client event handler crashed {}", e)
                        }
                    }
                    UserRequest::RoomMsgs(room_id) => match client.get_messages(&room_id).await {
                        Ok(res) => {
                            if let Err(e) = to_app
                                .send(RequestResult::RoomMsgs(Ok((
                                    res,
                                    Arc::clone(
                                        client
                                            .inner
                                            .joined_rooms()
                                            .read()
                                            .await
                                            .get(&room_id)
                                            .unwrap(),
                                    ),
                                ))))
                                .await
                            {
                                panic!("client event handler crashed {}", e)
                            } else {
                                // store state after receiving past events incase a sync_forever call only found a few messages
                                if client.store_room_state(&room_id).await.is_err() {
                                    // TODO log that an error happened at some point
                                }
                            }
                        }
                        Err(get_msg_err) => {
                            if let Err(e) = to_app.send(RequestResult::Error(get_msg_err)).await {
                                panic!("client event handler crashed {}", e)
                            }
                        }
                    },
                    UserRequest::AcceptInvite(room_id) => {
                        let res = client.join_room_by_id(&room_id).await;
                        if let Err(e) = to_app.send(RequestResult::AcceptInvite(res)).await {
                            panic!("client event handler crashed {}", e)
                        }
                    }
                    UserRequest::DeclineInvite(room_id) => {
                        let res = client.leave_room(&room_id).await;
                        if let Err(e) = to_app
                            .send(RequestResult::DeclineInvite(res, room_id))
                            .await
                        {
                            panic!("client event handler crashed {}", e)
                        }
                    }
                    UserRequest::LeaveRoom(room_id) => {
                        let res = client.leave_room(&room_id).await;
                        if let Err(e) = to_app
                            .send(RequestResult::LeaveRoom(res, room_id.clone()))
                            .await
                        {
                            panic!("client event handler crashed {}", e)
                        } else {
                            if let Err(e) = client.forget_room(&room_id).await {
                                panic!("client event handler crashed {}", e)
                            }
                        }
                    }
                    UserRequest::ReadReceipt(room_id, event_id) => {
                        let res = client.read_receipt(&room_id, &event_id).await;
                        if let Err(e) = to_app.send(RequestResult::ReadReceipt(res)).await {
                            panic!("client event handler crashed {}", e)
                        }
                    }
                    UserRequest::Typing(room_id, user_id) => {
                        let res = client
                            .typing_notice(
                                &room_id,
                                &user_id,
                                true,
                                Some(Duration::from_millis(3000)),
                            )
                            .await;
                        if let Err(e) = to_app.send(RequestResult::Typing(res)).await {
                            panic!("client event handler crashed {}", e)
                        }
                    }
                }
            }
        });

        (
            MatrixEventHandle {
                cli_jobs,
                sync_jobs,
                start_sync,
                quit_flag,
            },
            app_sender,
        )
    }

    /// This is called after login and initial sync to start `AsyncClient::sync_forever` loop.
    pub(crate) fn start_sync(&self) {
        self.start_sync
            .swap(true, std::sync::atomic::Ordering::SeqCst);
    }

    /// This is called when the user quits to signal the `tokio::Runtime` to shutdown.
    pub(crate) fn quit_sync(&self) {
        self.quit_flag
            .swap(true, std::sync::atomic::Ordering::SeqCst);
    }
}
