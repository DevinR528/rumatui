use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use anyhow::Result;
use matrix_sdk::Room;
use tokio::runtime::Handle;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Sender;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::client::event_stream::EventStream;
use crate::client::MatrixClient;
use matrix_sdk::api::r0::message::create_message_event;
use matrix_sdk::api::r0::message::get_message_events;
use matrix_sdk::events::{room::message::MessageEventContent, EventResult};
use matrix_sdk::identifiers::RoomId;
use matrix_sdk::{AsyncClient, AsyncClientConfig};

pub enum UserRequest {
    Login(String, String),
    SendMessage(RoomId, MessageEventContent),
    RoomMsgs(RoomId),
    Sync,
    Quit,
}
unsafe impl Send for UserRequest {}

impl fmt::Debug for UserRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Login(_, _) => write!(f, "failed login"),
            Self::SendMessage(id, _) => write!(f, "failed sending message for {}", id),
            Self::RoomMsgs(id) => write!(f, "failed to get room messages for {}", id),
            Self::Sync => write!(f, "syncing filed"),
            Self::Quit => write!(f, "quitting filed"),
        }
    }
}
pub enum RequestResult {
    Login(Result<HashMap<RoomId, Arc<Mutex<Room>>>>),
    SendMessage(Result<create_message_event::Response>),
    RoomMsgs(Result<get_message_events::IncomingResponse>),
    Error(anyhow::Error),
}
unsafe impl Send for RequestResult {}

pub struct MatrixEventHandle {
    cli_jobs: JoinHandle<Result<()>>,
}
unsafe impl Send for MatrixEventHandle {}

impl MatrixEventHandle {
    pub async fn new(
        stream: EventStream,
        mut to_app: Sender<RequestResult>,
        exec_hndl: Handle,
    ) -> (Self, Sender<UserRequest>) {
        let (app_sender, mut recv) = mpsc::channel(1024);

        let homeserver = "http://matrix.org";
        
        let mut c = MatrixClient::new(homeserver).unwrap();
        c.inner.add_event_emitter(Arc::new(Mutex::new(Box::new(stream))))
            .await;

        let client = Arc::new(Mutex::new(c));

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
                        let mut cli = client.lock().await;
                        let res = cli.login(u, p).await;
                        if let Err(e) = to_app.send(RequestResult::Login(res)).await {
                            panic!("client event handler crashed {}", e)
                        }
                    }
                    UserRequest::SendMessage(room, msg) => {
                        let mut cli = client.lock().await;
                        let res = cli.send_message(&room, msg).await;
                        if let Err(e) = to_app.send(RequestResult::SendMessage(res)).await {
                            panic!("client event handler crashed {}", e)
                        }
                    }
                    UserRequest::RoomMsgs(room_id) => {
                        let mut cli = client.lock().await;
                        match  cli.get_messages(&room_id).await {
                            Ok(mut res) => {
                                let base = cli.base_client();
                                let mut base = base.write().await;
                                for mut event in &mut res.chunk {
                                    base.receive_joined_timeline_event(&room_id, &mut event).await;
                    
                                    if let EventResult::Ok(e) = event {
                                        base.emit_timeline_event(&room_id, e).await;
                                    }
                                }
                                if let Err(e) = to_app.send(RequestResult::RoomMsgs(Ok(res))).await {
                                    panic!("client event handler crashed {}", e)
                                }
                            }
                            Err(get_msg_err) => {
                                if let Err(e) = to_app.send(RequestResult::Error(get_msg_err)).await {
                                    panic!("client event handler crashed {}", e)
                                }
                            }
                        }
                    }
                    UserRequest::Sync => {
                        let mut c = client.lock().await;
                        if let Err(sync_err) = c.sync().await {
                            if let Err(e) = to_app.send(RequestResult::Error(sync_err)).await {
                                panic!("client event handler crashed {}", e)
                            }
                        }
                    }
                }
            }
        });

        (MatrixEventHandle { cli_jobs }, app_sender)
    }
}
