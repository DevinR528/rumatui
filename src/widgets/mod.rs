use std::cell::RefCell;
use std::io;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, RwLock};

use anyhow::{Result, Context, Error};
use chrono::{offset::TimeZone, DateTime, Local};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use tokio::runtime::{Runtime, Handle};
use tokio::task::JoinHandle;
use tokio::sync::Mutex;
use tokio::sync::mpsc::{self, Sender, Receiver};
use tui::backend::Backend;
use tui::layout::{Constraint, Direction, Layout, Rect};
use tui::style::{Color, Modifier, Style};
use tui::widgets::{Block, Borders, Paragraph, Tabs, Text, Widget};
use tui::{Frame, Terminal};

pub mod chat;
mod login;
mod msgs;
mod rooms;
pub mod error;

use crate::client::{event_stream::{EventStream, StateResult}, MatrixClient};
use crate::client_loop::{MatrixEventHandle, UserRequest, RequestResult};
use chat::ChatWidget;
use login::{Login, LoginSelect, LoginWidget};
use error::ErrorWidget;

pub trait RenderWidget {
    fn render<B>(&mut self, f: &mut Frame<B>, area: Rect)
    where
        B: Backend;
}

pub trait DrawWidget {
    fn draw<B>(&mut self, terminal: &mut Terminal<B>) -> io::Result<()>
    where
        B: Backend + Send;
    fn draw_with<B>(&mut self, terminal: &mut Terminal<B>, area: Rect) -> io::Result<()>
    where
        B: Backend
    {
        Ok(())
    }
}

pub struct AppWidget {
    /// Title of the app "RumaTui".
    pub title: String,
    /// When user quits this is true,
    pub should_quit: bool,
    /// Have we started the sync loop yet.
    pub sync_started: bool,
    /// The login element. This knows how to render and also holds the state of logging in.
    pub login_w: LoginWidget,
    /// The main screen. Holds the state once a user is logged in.
    /// 
    /// ## Note
    /// Locking this may cause short deadlocks but the looping in the
    /// `matrix_sdk::AsyncClient:sync` should be short enough investigate.
    pub chat: ChatWidget,
    /// the event loop for MatrixClient tasks to run on.
    pub ev_loop: MatrixEventHandle,
    /// Send MatrixClient jobs to the event handler
    pub send_jobs: mpsc::Sender<UserRequest>,
    /// The result of any MatrixClient job.
    pub ev_msgs: mpsc::Receiver<RequestResult>,
    /// The result of any MatrixClient job.
    pub emitter_msgs: mpsc::Receiver<StateResult>,
    pub error: Option<anyhow::Error>,
}

impl AppWidget {
    pub async fn new(rt: Handle) -> Self {
        let (send, recv) = mpsc::channel(1024);
        
        let (emitter, emitter_msgs) = EventStream::new();

        let (ev_loop, send_jobs) = MatrixEventHandle::new(emitter, send, rt).await;
        Self {
            title: "RumaTui".to_string(),
            should_quit: false,
            sync_started: false,
            login_w: LoginWidget::default(),
            chat: ChatWidget::default(),
            ev_loop,
            send_jobs,
            ev_msgs: recv,
            emitter_msgs,
            error: None,
        }
    }

    pub fn on_up(&mut self) {
        if !self.login_w.logged_in {
            if let LoginSelect::Username = self.login_w.login.selected {
                self.login_w.login.selected = LoginSelect::Password;
            } else {
                self.login_w.login.selected = LoginSelect::Username;
            }
        }
    }

    pub fn on_down(&mut self) {
        if !self.login_w.logged_in {
            if let LoginSelect::Username = self.login_w.login.selected {
                self.login_w.login.selected = LoginSelect::Password;
            } else {
                self.login_w.login.selected = LoginSelect::Username;
            }
        }
    }

    pub fn on_right(&mut self) { }

    pub fn on_left(&mut self) { }

    async fn add_char(&mut self, c: char) {
        // TODO add homeserver_url sign in in client??
        if !self.login_w.logged_in {
            if c == '\n' {
                if self.login_w.try_login() {
                    let Login {
                        username, password, ..
                    } = &self.login_w.login;
                    self.login_w.logging_in = true;
                    if let Err(e) = self.send_jobs.send(UserRequest::Login(username.into(), password.into())).await {
                        self.set_error(Error::from(e));
                    }
                }
            }
            if let LoginSelect::Username = self.login_w.login.selected {
                self.login_w.login.username.push(c);
            } else {
                self.login_w.login.password.push(c);
            }
        }
    }

    pub async fn on_key(&mut self, c: char) {
        self.add_char(c).await;
    }

    pub fn on_backspace(&mut self) {
        if !self.login_w.logged_in {
            if let LoginSelect::Username = self.login_w.login.selected {
                self.login_w.login.username.pop();
            } else {
                self.login_w.login.password.pop();
            }
        }
    }

    pub fn on_delete(&mut self) {}

    fn set_error(&mut self, e: anyhow::Error) {
        self.error = Some(e);
    }

    /// This checks once then continues returns to continue the ui loop.
    pub async fn on_tick(&mut self) {
        match self.ev_msgs.try_recv() {
            Ok(res) => match res {
                RequestResult::Login(Ok(rooms)) => {
                    self.login_w.logged_in = true;
                    self.login_w.logging_in = false;
                    self.chat.set_room_state(rooms);
                },
                RequestResult::Login(Err(e)) => {
                    self.login_w.logging_in = false;
                    self.set_error(e)
                },
            }
            _ => {},
        }

        if self.login_w.logged_in && !self.sync_started {
            self.sync_started = true;
            self.ev_loop.start_sync();
        }
    }
}

impl DrawWidget for AppWidget {
    fn draw<B: Backend + Send>(&mut self, terminal: &mut Terminal<B>) -> io::Result<()> {
        terminal.draw(|mut f| {
            let chunks = Layout::default()
                .constraints([Constraint::Length(2), Constraint::Min(0)].as_ref())
                .split(f.size());

            Block::default()
                .borders(Borders::ALL)
                .title(&self.title)
                .title_style(Style::default().fg(Color::Green).modifier(Modifier::BOLD))
                .render(&mut f, chunks[0]);

            let chunks2 = Layout::default()
                .constraints([Constraint::Percentage(100)].as_ref())
                .split(chunks[1]);

            if let Some(err) = self.error.as_ref() {
                ErrorWidget::new(err).render(&mut f, chunks2[0])
            } else {
                if !self.login_w.logged_in {
                    self.login_w.render(&mut f, chunks2[0])
                } else {
                    self.chat.render(&mut f, chunks2[0])
                }
            }
        })
    }
}

#[allow(dead_code)]
mod task {
    use std::future::Future;
    use std::pin::Pin;
    use std::ptr;
    use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

    const RAW_WAKER: RawWaker = RawWaker::new(ptr::null(), &VTABLE);
    const VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_by_ref, drop);

    unsafe fn clone(_: *const ()) -> RawWaker {
        RAW_WAKER
    }

    unsafe fn wake(_: *const ()) {}

    unsafe fn wake_by_ref(_: *const ()) {}

    unsafe fn drop(_: *const ()) {}

    pub fn create() -> Waker {
        // Safety: The waker points to a vtable with functions that do nothing. Doing
        // is always safe.
        unsafe { Waker::from_raw(RAW_WAKER) }
    }

    pub fn block_on<F, T>(mut future: F) -> T
    where
        F: Future<Output = T>,
    {
        // Safety: since we own the future no one can move any part of it but us, and we won't.
        let mut fut = unsafe { Pin::new_unchecked(&mut future) };
        let waker = create();
        let mut ctx = Context::from_waker(&waker);
        loop {
            if let Poll::Ready(res) = fut.as_mut().poll(&mut ctx) {
                return res;
            }
            // TODO since criterion is single threaded simply looping seems ok
            // burning cpu for a simpler function seems fair
            // possible `std::sync::atomic::spin_loop_hint` here.
        }
    }
}
