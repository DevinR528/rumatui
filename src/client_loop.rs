use std::thread;
use std::time::Duration;
use std::ops::{Deref, DerefMut};
use std::marker::PhantomData;

use anyhow::{Result, Context};
use tokio::task::JoinHandle;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Sender, Receiver};
use tokio::runtime::Handle;

use crate::client::MatrixClient;

#[derive(Debug)]
pub enum UserRequest {
    Login(String, String),
    Quit,
}

pub enum RequestResult {
    Login(Result<()>),

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
                            panic!("client event handler chrashed {}", e)
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
