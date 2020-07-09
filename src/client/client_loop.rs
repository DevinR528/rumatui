use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use matrix_sdk::{
    api::r0::{
        account::register,
        directory::get_public_rooms_filtered::{self, RoomNetwork},
        membership::{join_room_by_id, leave_room},
        message::{create_message_event, get_message_events},
        // receipt::create_receipt,
        read_marker::set_read_marker,
        session::login,
        typing::create_typing_event,
    },
    events::room::message::MessageEventContent,
    identifiers::{EventId, RoomId, UserId},
    Room,
};
use tokio::{
    runtime::Handle,
    sync::{
        mpsc::{self, Sender},
        RwLock,
    },
    task::JoinHandle,
};
use uuid::Uuid;

use crate::{
    client::{event_stream::EventStream, MatrixClient},
    error::{Error, Result},
};

/// Requests sent from the UI portion of the app.
///
/// Each request is sent in response to some user input.
#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub enum UserRequest {
    Login(String, String),
    Register(String, String),
    SendMessage(RoomId, MessageEventContent, Uuid),
    RoomMsgs(RoomId),
    AcceptInvite(RoomId),
    DeclineInvite(RoomId),
    JoinRoom(RoomId),
    LeaveRoom(RoomId),
    Typing(RoomId, UserId),
    ReadReceipt(RoomId, EventId),
    RoomSearch(String, RoomNetwork, Option<String>),
    UiaaPing(String),
    UiaaDummy(String),
    Quit,
}
unsafe impl Send for UserRequest {}

/// Either a `UserRequest` succeeds or fails with the given result.
#[allow(clippy::type_complexity)]
pub enum RequestResult {
    Login(
        Result<(
            Arc<RwLock<HashMap<RoomId, Arc<RwLock<Room>>>>>,
            login::Response,
        )>,
    ),
    Register(Result<register::Response>),
    SendMessage(Result<create_message_event::Response>),
    RoomMsgs(Result<(get_message_events::Response, Arc<RwLock<Room>>)>),
    AcceptInvite(Result<join_room_by_id::Response>),
    DeclineInvite(Result<leave_room::Response>, RoomId),
    LeaveRoom(Result<leave_room::Response>, RoomId),
    JoinRoom(Result<RoomId>),
    Typing(Result<create_typing_event::Response>),
    ReadReceipt(Result<set_read_marker::Response>),
    RoomSearch(Result<get_public_rooms_filtered::Response>),
    Error(Error),
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
        // when the ui loop logs in `start_sync` releases and starts `sync_forever`
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
                            tracing::error!("client event handler crashed {}", e);
                            panic!("client event handler crashed {}", e)
                        }
                    }
                    UserRequest::Register(u, p) => {
                        let res = client.register_user(u, p).await;
                        if let Err(e) = to_app.send(RequestResult::Register(res)).await {
                            tracing::error!("client event handler crashed {}", e);
                            panic!("client event handler crashed {}", e)
                        } else {
                            tracing::info!("start UIAA cycle");
                        }
                    }
                    UserRequest::UiaaPing(sess) => {
                        let res = client.send_uiaa_ping(sess).await;
                        if let Err(e) = to_app
                            .send(RequestResult::Register(res.map(Into::into)))
                            .await
                        {
                            tracing::error!("client event handler crashed {}", e);
                            panic!("client event handler crashed {}", e)
                        } else {
                            tracing::info!("ping UIAA endpoint");
                        }
                    }
                    UserRequest::UiaaDummy(sess) => {
                        let res = client.send_uiaa_dummy(sess).await;
                        if let Err(e) = to_app
                            .send(RequestResult::Register(res.map(Into::into)))
                            .await
                        {
                            tracing::error!("client event handler crashed {}", e);
                            panic!("client event handler crashed {}", e)
                        } else {
                            tracing::info!("sending the dummy UIAA request");
                        }
                    }
                    UserRequest::SendMessage(room, msg, uuid) => {
                        let res = client.send_message(&room, msg, uuid).await;
                        if let Err(e) = to_app.send(RequestResult::SendMessage(res)).await {
                            tracing::error!("client event handler crashed {}", e);
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
                                tracing::error!("client event handler crashed {}", e);
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
                                tracing::error!("client event handler crashed {}", e);
                                panic!("client event handler crashed {}", e)
                            }
                        }
                    },
                    UserRequest::RoomSearch(filter, network, tkn) => {
                        match client.get_rooms_filtered(&filter, network, tkn).await {
                            Ok(res) => {
                                if let Err(e) =
                                    to_app.send(RequestResult::RoomSearch(Ok(res))).await
                                {
                                    tracing::error!("client event handler crashed {}", e);
                                    panic!("client event handler crashed {}", e)
                                }
                            }
                            Err(err) => {
                                if let Err(e) = to_app.send(RequestResult::Error(err)).await {
                                    tracing::error!("client event handler crashed {}", e);
                                    panic!("client event handler crashed {}", e)
                                }
                            }
                        }
                    }
                    UserRequest::AcceptInvite(room_id) => {
                        let res = client.join_room_by_id(&room_id).await;
                        if let Err(e) = to_app.send(RequestResult::AcceptInvite(res)).await {
                            tracing::error!("client event handler crashed {}", e);
                            panic!("client event handler crashed {}", e)
                        }
                    }
                    UserRequest::DeclineInvite(room_id) => {
                        let res = client.leave_room(&room_id).await;
                        if let Err(e) = to_app
                            .send(RequestResult::DeclineInvite(res, room_id))
                            .await
                        {
                            tracing::error!("client event handler crashed {}", e);
                            panic!("client event handler crashed {}", e)
                        }
                    }
                    UserRequest::LeaveRoom(room_id) => {
                        let res = client.leave_room(&room_id).await;
                        if let Err(e) = to_app
                            .send(RequestResult::LeaveRoom(res, room_id.clone()))
                            .await
                        {
                            tracing::error!("client event handler crashed {}", e);
                            panic!("client event handler crashed {}", e)
                        } else if let Err(error) = client.forget_room(&room_id).await {
                            // forget room failed so send that to the UI
                            if let Err(e) = to_app.send(RequestResult::Error(error)).await {
                                tracing::error!("client event handler crashed {}", e);
                                panic!("client event handler crashed {}", e)
                            }
                        }
                    }
                    UserRequest::JoinRoom(room_id) => {
                        // TODO just send the result
                        match client.join_room_by_id(&room_id).await {
                            Ok(res) => {
                                let room_id = &res.room_id;
                                if let Err(e) = to_app
                                    .send(RequestResult::JoinRoom(Ok(room_id.clone())))
                                    .await
                                {
                                    tracing::error!("client event handler crashed {}", e);
                                    panic!("client event handler crashed {}", e)
                                }
                            }
                            Err(err) => {
                                if let Err(e) = to_app.send(RequestResult::JoinRoom(Err(err))).await
                                {
                                    tracing::error!("client event handler crashed {}", e);
                                    panic!("client event handler crashed {}", e)
                                }
                            }
                        }
                    }
                    UserRequest::ReadReceipt(room_id, event_id) => {
                        let res = client
                            .read_marker(&room_id, &event_id, Some(&event_id))
                            .await;
                        if let Err(e) = to_app.send(RequestResult::ReadReceipt(res)).await {
                            tracing::error!("client event handler crashed {}", e);
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
                            tracing::error!("client event handler crashed {}", e);
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
