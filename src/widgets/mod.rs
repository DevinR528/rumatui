use std::cell::RefCell;
use std::io;
use std::process::{Child, Command, Stdio};

use anyhow::{Result, Context, Error};
use chrono::{offset::TimeZone, DateTime, Local};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use tokio::runtime::{Runtime, Handle};
use tokio::task::JoinHandle;
use tokio::sync::mpsc;
use tokio::sync::mpsc::{Sender, Receiver};
use tui::backend::Backend;
use tui::layout::{Constraint, Direction, Layout, Rect};
use tui::style::{Color, Modifier, Style};
use tui::widgets::{Block, Borders, Paragraph, Tabs, Text, Widget};
use tui::{Frame, Terminal};

mod chat;
mod login;
mod msgs;
mod rooms;
pub mod error;

use crate::client::MatrixClient;
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
        B: Backend;
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
    /// The login element. This knows how to render and also holds the state of logging in.
    pub login_w: LoginWidget,
    /// The main screen. Holds the state once a user is logged in.
    pub chat: ChatWidget,
    /// the event loop for MatrixClient tasks to run on.
    pub ev_loop: MatrixEventHandle,
    /// Send MatrixClient jobs to the event handler
    pub send_jobs: mpsc::Sender<UserRequest>,
    /// The result of any MatrixClient job.
    pub client_jobs: mpsc::Receiver<RequestResult>,
    pub error: Option<anyhow::Error>,
}

impl AppWidget {
    pub fn new(rt: Handle) -> Self {
        let (send, recv) = mpsc::channel(1024);
        let (ev_loop, send_jobs) = MatrixEventHandle::new(send, rt);
        Self {
            title: "RumaTui".to_string(),
            should_quit: false,
            login_w: LoginWidget::default(),
            chat: ChatWidget::default(),
            ev_loop,
            send_jobs,
            client_jobs: recv,
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
                        println!("CHANNEL ERROR");
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
    pub fn on_tick(&mut self) {
        match self.client_jobs.try_recv() {
            Ok(res) => match res {
                RequestResult::Login(Ok(_)) => {
                    self.login_w.logged_in = true;
                    self.login_w.logging_in = false;
                },
                RequestResult::Login(Err(e)) => {
                    println!("ERRORRRRRRRR");
                    self.login_w.logging_in = false;
                    self.set_error(e)
                },
            }
            _ => {},
        }
    }
}

impl DrawWidget for AppWidget {
    fn draw<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> io::Result<()> {
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

mod date_fmt {
    use super::*;

    const FORMAT: &str = "%Y-%m-%d %H:%M:%S";

    pub fn serialize<S>(date: &DateTime<Local>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = format!("{}", date.format(FORMAT));
        serializer.serialize_str(&s)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Local>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Local
            .datetime_from_str(&s, FORMAT)
            .map_err(serde::de::Error::custom)
    }
}
