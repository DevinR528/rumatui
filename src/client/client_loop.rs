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
    sync_jobs: JoinHandle<Result<()>>,
    start_sync: Arc<AtomicBool>,
    quit_flag: Arc<AtomicBool>,
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
        let cli = Arc::clone(&client);

        // when the ui loop logs in start_sync releases and starts `sync_forever`
        let start_sync = Arc::from(AtomicBool::from(false));
        let quit_flag = Arc::from(AtomicBool::from(false));

        let is_sync = Arc::clone(&start_sync);
        let quitting = Arc::clone(&quit_flag);
        // this loop uses the above `AtomicBool` to signal shutdown.
        let sync_jobs = exec_hndl.spawn(async move {
            // while !is_sync.load(Ordering::SeqCst) {
            //     if quitting.load(Ordering::SeqCst) {
            //         return Ok(());
            //     }

            //     std::sync::atomic::spin_loop_hint();
            // }

            // if quitting.load(Ordering::SeqCst) {
            //     return Ok(());
            // }

            // let set = matrix_sdk::SyncSettings::default();
            // let mut c = cli.lock().await;
            // c.inner.sync_forever(set.clone(), |_| async {}).await;
            Ok(())
        });

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
