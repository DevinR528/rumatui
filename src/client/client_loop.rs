use std::collections::HashMap;
use std::fmt;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

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
use matrix_sdk::api::r0::message::create_message_event;
use matrix_sdk::api::r0::message::get_message_events;
use matrix_sdk::api::r0::session::login;
use matrix_sdk::events::room::message::MessageEventContent;
use matrix_sdk::identifiers::RoomId;

pub enum UserRequest {
    Login(String, String),
    SendMessage(RoomId, MessageEventContent, Uuid),
    RoomMsgs(RoomId),
    Quit,
}
unsafe impl Send for UserRequest {}

impl fmt::Debug for UserRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Login(name, _) => write!(f, "failed login for {}", name),
            Self::SendMessage(id, _, _) => write!(f, "failed sending message for {}", id),
            Self::RoomMsgs(id) => write!(f, "failed to get room messages for {}", id),
            Self::Quit => write!(f, "quitting filed"),
        }
    }
}
pub enum RequestResult {
    Login(Result<(HashMap<RoomId, Arc<RwLock<Room>>>, login::Response)>),
    SendMessage(Result<create_message_event::Response>),
    RoomMsgs(Result<(get_message_events::Response, Arc<RwLock<Room>>)>),
    Error(anyhow::Error),
}
unsafe impl Send for RequestResult {}

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
                                        client.inner.get_rooms().await.get(&room_id).unwrap(),
                                    ),
                                ))))
                                .await
                            {
                                panic!("client event handler crashed {}", e)
                            }
                        }
                        Err(get_msg_err) => {
                            if let Err(e) = to_app.send(RequestResult::Error(get_msg_err)).await {
                                panic!("client event handler crashed {}", e)
                            }
                        }
                    },
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
