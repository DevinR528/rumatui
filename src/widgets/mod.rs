use std::cell::RefCell;
use std::io;
use std::ops::{Index, IndexMut};
use std::process::{Child, Command, Stdio};
use std::thread;

use chrono::{offset::TimeZone, DateTime, Local};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use tokio::runtime::Runtime;
use tui::backend::Backend;
use tui::layout::{Constraint, Direction, Layout, Rect};
use tui::style::{Color, Modifier, Style};
use tui::widgets::{Block, Borders, Paragraph, Tabs, Text, Widget};
use tui::{Frame, Terminal};

mod login;
mod messages;

use crate::client::MatrixClient;
use login::{Login, LoginSelect, LoginWidget};
use messages::MessageWidget;

pub trait RenderWidget {
    fn render<B>(&mut self, f: &mut Frame<B>, area: Rect)
    where
        B: Backend;
}

pub trait DrawWidget {
    fn draw<B>(&mut self, terminal: &mut Terminal<B>) -> io::Result<()>
    where
        B: Backend;
}

#[derive(Clone, Debug, Default)]
pub struct TabsState {
    pub titles: Vec<String>,
    pub index: usize,
}

impl TabsState {
    pub fn new(titles: Vec<String>) -> TabsState {
        TabsState { titles, index: 0 }
    }
    pub fn next(&mut self) {
        self.index = (self.index + 1) % self.titles.len();
    }

    pub fn previous(&mut self) {
        if !self.titles.is_empty() {
            if self.index > 0 {
                self.index -= 1;
            } else {
                self.index = self.titles.len() - 1;
            }
        }
    }
}

#[derive(Debug)]
pub struct AppWidget {
    pub title: String,
    pub tabs: TabsState,
    pub should_quit: bool,
    pub login_w: LoginWidget,
    pub messages: MessageWidget,
    pub client: Option<MatrixClient>,
    pub cmd_handle: RefCell<Vec<thread::JoinHandle<Result<Child, io::Error>>>>,
    pub cmd_err: String,
}

impl AppWidget {
    pub async fn new() -> Result<Self, failure::Error> {
        Ok(Self {
            title: "RumaTui".to_string(),
            tabs: TabsState::default(),
            should_quit: false,
            login_w: LoginWidget::default(),
            messages: MessageWidget::default(),
            client: None,
            cmd_handle: Default::default(),
            cmd_err: String::default(),
        })
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
    /// TODO should any addition be reset here?
    pub fn on_right(&mut self) {
        self.reset_addition();
        self.tabs.next();
    }

    /// TODO should any addition be reset here?
    pub fn on_left(&mut self) {
        self.reset_addition();
        self.tabs.previous();
    }

    fn reset_addition(&mut self) {}

    fn run_cmd(&self, cmd: String) {}

    async fn add_char(&mut self, c: char) {
        // TODO add homeserver_url sign in??
        if !self.login_w.logged_in {
            self.client = Some(MatrixClient::new("http://matrix.org").await.unwrap());
            if c == '\n' {
                if let LoginSelect::Password = self.login_w.login.selected {
                    if !self.login_w.login.password.is_empty()
                        && !self.login_w.login.username.is_empty()
                    {
                        let Login {
                            username, password, ..
                        } = &self.login_w.login;

                        let res = self
                            .client
                            .as_mut()
                            .unwrap()
                            .login(username.into(), password.into())
                            .await;

                        if res.is_ok() {
                            self.login_w.logged_in = true;
                            println!("SIGNED IN");
                        } else {
                            panic!("{:?}", res);
                        }
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

    pub fn on_tick(&mut self) {
        // self.cmd_handle
    }
}

impl DrawWidget for AppWidget {
    fn draw<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> io::Result<()> {
        terminal.draw(|mut f| {
            let chunks = Layout::default()
                .constraints([Constraint::Length(2), Constraint::Min(0)].as_ref())
                .split(f.size());

            Tabs::default()
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(&self.title)
                        .title_style(Style::default().fg(Color::Green).modifier(Modifier::BOLD)),
                )
                .titles(&self.tabs.titles)
                .style(Style::default().fg(Color::Blue))
                .highlight_style(Style::default().fg(Color::Blue).modifier(Modifier::ITALIC))
                .select(self.tabs.index)
                .render(&mut f, chunks[0]);

            let chunks2 = Layout::default()
                .constraints([Constraint::Percentage(100)].as_ref())
                .split(chunks[1]);

            if !self.login_w.logged_in {
                self.login_w.render(&mut f, chunks2[0]);
            } else {
                self.messages.render(&mut f, chunks2[0]);
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
