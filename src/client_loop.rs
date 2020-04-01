use std::fmt;




use std::collections::HashMap;
use std::sync::{atomic::AtomicBool, Arc, RwLock};

use anyhow::{Result};
use matrix_sdk::{Room};
use tokio::task::JoinHandle;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Sender};
use tokio::runtime::Handle;
use tokio::sync::Mutex;

use crate::client::MatrixClient;
use crate::client::event_stream::{EventStream};

pub enum UserRequest {
    Login(String, String),
    Quit,
}
unsafe impl Send for UserRequest {}

impl fmt::Debug for UserRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Login(_, _) => write!(f, "failed login"),
            Self::Quit => write!(f, "quitting filed")
        }
    }
}
pub enum RequestResult {
    Login(Result<HashMap<String, Arc<RwLock<Room>>>>),

}
unsafe impl Send for RequestResult {}

pub struct MatrixEventHandle {
    cli_jobs: JoinHandle<Result<()>>,
    sync_jobs: JoinHandle<Result<()>>,
    start_sync: Arc<AtomicBool>,
}
unsafe impl Send for MatrixEventHandle {}

impl MatrixEventHandle {
    pub async fn new(stream: EventStream, to_app: Sender<RequestResult>, exec_hndl: Handle) -> (Self, Sender<UserRequest>) {
        let (app_sender, mut recv) = mpsc::channel(1024);

        let mut tx = to_app.clone();

        let mut c = MatrixClient::new("http://matrix.org").unwrap();
        c.inner.add_event_emitter(Arc::new(Mutex::new(Box::new(stream)))).await;

        let client = Arc::new(Mutex::new(c));
        let cli = Arc::clone(&client);

        // when the ui loop logs in start_sync releases and starts `sync_forever`
        let start_sync = Arc::from(AtomicBool::from(false));
        let is_sync = Arc::clone(&start_sync);
        let sync_jobs = exec_hndl.spawn(async move {
            while !is_sync.load(std::sync::atomic::Ordering::SeqCst) {
                std::sync::atomic::spin_loop_hint();
            }
            let set = matrix_sdk::SyncSettings::default();
            let mut c = cli.lock().await;
            c.sync_forever(set).await
        });

        let cli_jobs = exec_hndl.spawn(async move {
            for input in recv.recv().await {
                let input: UserRequest = input;
                match input {
                    UserRequest::Quit => return Ok(()),
                    UserRequest::Login(u, p) => {
                        let mut cli = client.lock().await;
                        let res = cli.login(u, p).await;
                        if let Err(e) = tx.send(RequestResult::Login(res)).await {
                            panic!("client event handler crashed {}", e)
                        }
                    },
                }
            }
            Ok(())
        });

        (
            MatrixEventHandle {
                cli_jobs,
                sync_jobs,
                start_sync,
            },
            app_sender,
        )
    }
    
    pub(crate) fn start_sync(&self) {
        self.start_sync.swap(true, std::sync::atomic::Ordering::SeqCst);
    }
}
