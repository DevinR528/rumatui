use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::Ordering;
use std::sync::{atomic::AtomicBool, Arc};

use anyhow::Result;
use matrix_sdk::Room;
use tokio::runtime::Handle;
use tokio::sync::mpsc;
use tokio::sync::mpsc::Sender;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

use crate::client::event_stream::EventStream;
use crate::client::MatrixClient;
use matrix_sdk::identifiers::RoomId;
use matrix_sdk::events::room::message::{MessageEventContent, };
use matrix_sdk::api::r0::message::create_message_event as create_msg;
pub enum UserRequest {
    Login(String, String),
    SendMessage(RoomId, MessageEventContent),
    Sync,
    Quit,
}
unsafe impl Send for UserRequest {}

impl fmt::Debug for UserRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Login(_, _) => write!(f, "failed login"),
            Self::SendMessage(_, _) => write!(f, "failed sending message"),
            Self::Sync => write!(f, "syncing filed"),
            Self::Quit => write!(f, "quitting filed"),
        }
    }
}
pub enum RequestResult {
    Login(Result<HashMap<RoomId, Arc<Mutex<Room>>>>),
    SendMessage(Result<create_msg::Response>)
}
unsafe impl Send for RequestResult {}

pub struct MatrixEventHandle {
    cli_jobs: JoinHandle<Result<()>>,
}
unsafe impl Send for MatrixEventHandle {}

impl MatrixEventHandle {
    pub async fn new(
        stream: EventStream,
        to_app: Sender<RequestResult>,
        exec_hndl: Handle,
    ) -> (Self, Sender<UserRequest>) {
        let (app_sender, mut recv) = mpsc::channel(1024);

        let mut tx = to_app.clone();

        let mut c = MatrixClient::new("http://matrix.org").unwrap();
        c.inner
            .add_event_emitter(Arc::new(Mutex::new(Box::new(stream))))
            .await;

        let client = Arc::new(Mutex::new(c));

        // this loop is shutdown with a channel message
        let cli_jobs = exec_hndl.spawn(async move {
            loop {
                let input = recv.recv().await;
                if input.is_none() { return Ok(()); }

                match input.unwrap() {
                    UserRequest::Quit => return Ok(()),
                    UserRequest::Login(u, p) => {
                        let mut cli = client.lock().await;
                        let res = cli.login(u, p).await;
                        if let Err(e) = tx.send(RequestResult::Login(res)).await {
                            panic!("client event handler crashed {}", e)
                        }
                    }
                    UserRequest::SendMessage(room, msg) => {
                        let mut cli = client.lock().await;
                        let res = cli.send_message(&room, msg).await;
                        if let Err(e) = tx.send(RequestResult::SendMessage(res)).await {
                            panic!("client event handler crashed {}", e)
                        }
                    }
                    UserRequest::Sync => {
                        let mut c = client.lock().await;
                        c.sync().await;
                    }
                    _ => {},
                    
                }
            }
        });

        (
            MatrixEventHandle {
                cli_jobs,
            },
            app_sender,
        )
    }
}
