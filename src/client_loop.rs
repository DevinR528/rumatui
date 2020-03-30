use std::fmt;
use std::thread;
use std::time::Duration;
use std::ops::{Deref, DerefMut};
use std::marker::PhantomData;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

use anyhow::{Result, Context};
use matrix_sdk::{EventEmitter, Room};
use tokio::task::JoinHandle;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Sender, Receiver};
use tokio::runtime::Handle;

use crate::client::MatrixClient;

pub enum UserRequest {
    Login(String, String),
    Sync(Arc<Mutex<crate::widgets::chat::ChatWidget>>),
    Quit,
}
unsafe impl Send for UserRequest {}

impl fmt::Debug for UserRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Login(_, _) => write!(f, "failed login"),
            Self::Sync(_) => write!(f, "sync failed"),
            Self::Quit => write!(f, "quitting filed")
        }
    }
}
pub enum RequestResult {
    Login(Result<HashMap<String, Arc<RwLock<Room>>>>),
    Sync(Result<()>)

}
unsafe impl Send for RequestResult {}

pub struct MatrixEventHandle {
    cli_jobs: JoinHandle<Result<()>>,
}

impl MatrixEventHandle {
    pub fn new(mut to_app: Sender<RequestResult>, exec_hndl: Handle) -> (Self, Sender<UserRequest>) {
        let (app_sender, mut recv) = mpsc::channel(1024);

        let mut tx = to_app.clone();
        let cli_jobs = exec_hndl.spawn(async move {

            let mut client = MatrixClient::new("http://matrix.org").unwrap();

            for input in recv.recv().await {
                let input: UserRequest = input;
                match input {
                    UserRequest::Quit => return Ok(()),
                    UserRequest::Login(u, p) => {
                        if let Err(e) = tx.send(RequestResult::Login(client.login(u, p).await)).await {
                            panic!("client event handler crashed {}", e)
                        }
                    }
                    UserRequest::Sync(ee) => {
                        let settings = matrix_sdk::SyncSettings::default();
                        if let Err(e) = tx.send(RequestResult::Sync(client.sync(settings, ee).await)).await {
                            panic!("client event handler crashed {}", e)
                        }
                    }
                }
            }
            Ok(())
        });

        (
            MatrixEventHandle {
                cli_jobs,
            },
            app_sender,
        )
    }
}
