use std::io;

use anyhow::Error;
use tokio::runtime::Handle;

use termion::event::MouseButton;
use tokio::sync::mpsc::{self};
use tui::backend::Backend;
use tui::layout::{Constraint, Layout, Rect};
use tui::style::{Color, Modifier, Style};
use tui::widgets::{Block, Borders};
use tui::{Frame, Terminal};

use super::chat::ChatWidget;
use super::error::ErrorWidget;
use super::login::{Login, LoginSelect, LoginWidget};
use crate::client::client_loop::{MatrixEventHandle, RequestResult, UserRequest};
use crate::client::event_stream::{EventStream, StateResult};

pub trait RenderWidget {
    fn render<B>(&mut self, f: &mut Frame<B>, area: Rect)
    where
        B: Backend;
}

pub trait DrawWidget {
    fn draw<B>(&mut self, terminal: &mut Terminal<B>) -> io::Result<()>
    where
        B: Backend + Send;
    fn draw_with<B>(&mut self, _terminal: &mut Terminal<B>, _area: Rect) -> io::Result<()>
    where
        B: Backend,
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
    /// The number of ticks since last sync
    pub ticks: usize,
    /// The login element. This knows how to render and also holds the state of logging in.
    pub login_w: LoginWidget,
    /// The main screen. Holds the state once a user is logged in.
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
            ticks: 0,
            login_w: LoginWidget::default(),
            chat: ChatWidget::default(),
            ev_loop,
            send_jobs,
            ev_msgs: recv,
            emitter_msgs,
            error: None,
        }
    }

    pub fn on_click(&mut self, btn: MouseButton, x: u16, y: u16) {
        if !self.login_w.logged_in {
            self.login_w.on_click(btn, x, y);
        }

        self.chat.room.on_click(btn, x, y)
    }

    pub async fn on_scroll_up(&mut self, x: u16, y: u16) {
        if self.chat.main_screen {
            if self.chat.msgs.on_scroll_up(x, y) {
                let room_id = self.chat.room.current_room.borrow().as_ref().unwrap().clone();
                if let Err(e) = self
                        .send_jobs
                        .send(UserRequest::RoomMsgs(room_id))
                        .await
                    {
                        self.set_error(anyhow::Error::from(e))
                    }
            }
        }
    }

    pub fn on_up(&mut self) {
        if !self.login_w.logged_in {
            if let LoginSelect::Username = self.login_w.login.selected {
                self.login_w.login.selected = LoginSelect::Password;
            } else {
                self.login_w.login.selected = LoginSelect::Username;
            }
        } else if self.chat.main_screen {
            self.chat.room.select_previous()
        }
    }

    pub fn on_down(&mut self) {
        if !self.login_w.logged_in {
            if let LoginSelect::Username = self.login_w.login.selected {
                self.login_w.login.selected = LoginSelect::Password;
            } else {
                self.login_w.login.selected = LoginSelect::Username;
            }
        } else if self.chat.main_screen {
            self.chat.room.select_next()
        }
    }

    pub fn on_right(&mut self) {}

    pub fn on_left(&mut self) {}

    async fn add_char(&mut self, c: char) {
        // TODO add homeserver_url sign in in client??
        if !self.login_w.logged_in {
            if c == '\n' && self.login_w.try_login() {
                let Login {
                    username, password, ..
                } = &self.login_w.login;
                self.login_w.logging_in = true;
                if let Err(e) = self
                    .send_jobs
                    .send(UserRequest::Login(username.into(), password.into()))
                    .await
                {
                    self.set_error(Error::from(e));
                }
            }
            if let LoginSelect::Username = self.login_w.login.selected {
                self.login_w.login.username.push(c);
            } else {
                self.login_w.login.password.push(c);
            }
        } else if self.chat.main_screen {
            self.chat.msgs.add_char(c);
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
        } else if self.chat.main_screen {
            self.chat.msgs.pop();
        }
    }

    pub fn on_delete(&mut self) {}

    pub async fn on_send(&mut self) {
        // unfortunately we have to do it this way or we have a mutable borrow in the scope of immutable
        let res = if let Some(room) = self.chat.current_room.borrow().as_ref() {
            match self.chat.msgs.get_sending_message() {
                Ok(msg) => {
                    if let Err(e) = self
                        .send_jobs
                        .send(UserRequest::SendMessage(room.clone(), msg))
                        .await
                    {
                        Err(anyhow::Error::from(e))
                    } else {
                        Ok(())
                    }
                }
                Err(e) => Err(e),
            }
        } else {
            Ok(())
        };
        if let Err(e) = res {
            self.set_error(Error::from(e));
        }
    }

    fn set_error(&mut self, e: anyhow::Error) {
        self.error = Some(e);
    }

    /// This checks once then continues returns to continue the ui loop.
    pub async fn on_tick(&mut self) {
        if self.login_w.logged_in && !self.sync_started {
            self.sync_started = true;
        }
        // this will login, send messages, and any other user initiated requests
        match self.ev_msgs.try_recv() {
            Ok(res) => match res {
                RequestResult::Login(Ok(rooms)) => {
                    self.login_w.logged_in = true;
                    self.chat.main_screen = true;
                    self.login_w.logging_in = false;
                    self.chat.set_room_state(rooms).await;
                }
                RequestResult::Login(Err(e)) => {
                    self.login_w.logging_in = false;
                    self.set_error(e)
                }
                // TODO this has the EventId which we need to keep
                RequestResult::SendMessage(Ok(_res)) => { println!("msg sent") }
                RequestResult::SendMessage(Err(e)) => self.set_error(e),
                RequestResult::RoomMsgs(Ok(_res)) => {  },
                RequestResult::RoomMsgs(Err(e)) => self.set_error(e),

                // sync error
                RequestResult::Error(err) => self.set_error(err),
            },
            _ => {}
        }

        match self.emitter_msgs.try_recv() {
            Ok(res) => match res {
                StateResult::Message(msg, room) => self.chat.msgs.add_message(msg, room),
                _ => {}
            },
            _ => {}
        }

        if self.sync_started && self.ticks == 10 {
            self.ticks = 0;
            if let Err(e) = self.send_jobs.send(UserRequest::Sync).await {
                self.set_error(Error::from(e));
            }
        }

        // increment tick counter for next sync
        if self.sync_started {
            self.ticks += 1;
        }
    }

    pub async fn on_quit(&mut self) {
        if self.send_jobs.send(UserRequest::Quit).await.is_err() {
            // TODO what should happen when a send fails
            return;
        };
    }
}

impl DrawWidget for AppWidget {
    fn draw<B: Backend + Send>(&mut self, terminal: &mut Terminal<B>) -> io::Result<()> {
        terminal.draw(|mut f| {
            let chunks = Layout::default()
                .constraints([Constraint::Length(2), Constraint::Min(0)].as_ref())
                .split(f.size());

            let blk = Block::default()
                .borders(Borders::ALL)
                .title(&self.title)
                .title_style(Style::default().fg(Color::Green).modifier(Modifier::BOLD));

            f.render_widget(blk, chunks[0]);

            let chunks2 = Layout::default()
                .constraints([Constraint::Percentage(100)].as_ref())
                .split(chunks[1]);

            if let Some(err) = self.error.as_ref() {
                ErrorWidget::new(err).render(&mut f, chunks2[0])
            } else if !self.login_w.logged_in {
                self.login_w.render(&mut f, chunks2[0])
            } else {
                self.chat.render(&mut f, chunks2[0])
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
