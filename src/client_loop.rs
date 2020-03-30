use std::fmt;
use std::thread;
use std::time::Duration;
use std::ops::{Deref, DerefMut};
use std::marker::PhantomData;
use std::collections::HashMap;
use std::sync::{atomic::AtomicBool, Arc, RwLock};

use anyhow::{Result, Context};
use matrix_sdk::{EventEmitter, Room};
use tokio::task::JoinHandle;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Sender, Receiver};
use tokio::runtime::Handle;
use tokio::sync::Mutex;
use tokio::sync::Barrier;

use crate::client::MatrixClient;

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
    pub fn new(mut to_app: Sender<RequestResult>, exec_hndl: Handle) -> (Self, Sender<UserRequest>) {
        let (app_sender, mut recv) = mpsc::channel(1024);

        let mut tx = to_app.clone();

        let mut client = Arc::new(Mutex::new(MatrixClient::new("http://matrix.org").unwrap()));

        let cli = Arc::clone(&client);

        // when the ui loop logs in start_sync releases and starts `sync_forever`
        let start_sync = Arc::from(AtomicBool::from(false));
        let is_sync = Arc::clone(&start_sync);
        let sync_jobs = exec_hndl.spawn(async move {
            while !is_sync.load(std::sync::atomic::Ordering::SeqCst) {
                std::sync::atomic::spin_loop_hint();
            }
            println!("START");
            let set = matrix_sdk::SyncSettings::default();
            cli.lock().await.sync(set).await
        });

        let cli_jobs = exec_hndl.spawn(async move {
            for input in recv.recv().await {
                let input: UserRequest = input;
                match input {
                    UserRequest::Quit => return Ok(()),
                    UserRequest::Login(u, p) => {
                        if let Err(e) = tx.send(RequestResult::Login(client.lock().await.login(u, p).await)).await {
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
