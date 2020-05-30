use std::io;
use std::ops::Deref;
use std::sync::Arc;
use std::time::SystemTime;

use anyhow::Error;
use matrix_sdk::api::r0::message::get_message_events;
use matrix_sdk::events::{
    collections::all::RoomEvent,
    room::{
        member::MembershipChange,
        message::{MessageEvent, MessageEventContent, TextMessageEventContent},
    },
};
use matrix_sdk::Room;
use rumatui_tui::backend::Backend;
use rumatui_tui::layout::{Alignment, Constraint, Layout};
use rumatui_tui::style::{Color, Modifier, Style};
use rumatui_tui::widgets::{Block, Borders, Paragraph, Text};
use rumatui_tui::Terminal;
use termion::event::MouseButton;
use tokio::runtime::Handle;
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

use crate::client::client_loop::{MatrixEventHandle, RequestResult, UserRequest};
use crate::client::event_stream::{EventStream, StateResult};
use crate::widgets::{
    chat::ChatWidget,
    error::ErrorWidget,
    login::{Login, LoginSelect, LoginWidget},
    message::Message,
    rooms::Invite,
    DrawWidget, RenderWidget,
};

// TODO split AppWidget into render and state halves. AppRender has the methods to deal with rendering.
// AppState or AppData?? will delegate to the state half of each widget.

pub struct AppWidget {
    /// Title of the app "rumatui".
    pub title: String,
    /// The address of the homeserver.
    pub homeserver: String,
    /// When user quits this is true,
    pub should_quit: bool,
    /// Have we started the sync loop yet.
    pub sync_started: bool,
    /// Have we started a scroll request.
    pub scrolling: bool,
    /// The client is sending a typing notice to the server.
    pub typing_notice: bool,
    /// The last interaction the user had with the app.
    pub last_interaction: SystemTime,
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
    pub async fn new(rt: Handle, homeserver: &str) -> Self {
        let homeserver = if homeserver.is_empty() {
            "http://matrix.org"
        } else {
            homeserver
        };

        let (send, recv) = mpsc::channel(1024);

        let (emitter, emitter_msgs) = EventStream::new();

        let (ev_loop, send_jobs) = MatrixEventHandle::new(emitter, send, rt, homeserver).await;
        Self {
            title: "rumatui".to_string(),
            homeserver: homeserver.to_string(),
            should_quit: false,
            sync_started: false,
            scrolling: false,
            typing_notice: false,
            last_interaction: SystemTime::now(),
            login_w: LoginWidget::default(),
            chat: ChatWidget::default(),
            ev_loop,
            send_jobs,
            ev_msgs: recv,
            emitter_msgs,
            error: None,
        }
    }

    pub async fn on_click(&mut self, btn: MouseButton, x: u16, y: u16) {
        if !self.login_w.logged_in {
            self.login_w.on_click(btn, x, y);
        }
        if self.chat.msgs.on_click(btn, x, y) {
            self.on_send().await;
        }
        if let Some(room_id) = self.chat.room.invite.as_ref().map(|i| i.room_id.clone()) {
            match self.chat.room.on_click(btn, x, y) {
                Invite::Accept => {
                    if let Err(e) = self
                        .send_jobs
                        .send(UserRequest::AcceptInvite(room_id))
                        .await
                    {
                        self.set_error(anyhow::Error::from(e))
                    } else {
                        self.chat.joining_room = true;
                        self.chat.room.remove_invite();
                    }
                }
                Invite::Decline => {
                    if let Err(e) = self
                        .send_jobs
                        .send(UserRequest::DeclineInvite(room_id))
                        .await
                    {
                        self.set_error(anyhow::Error::from(e))
                    } else {
                        self.chat.room.remove_invite();
                    }
                }
                Invite::NoClick => {}
            }
        }
    }

    pub async fn on_scroll_up(&mut self, x: u16, y: u16) {
        if self.chat.main_screen {
            if self.chat.msgs.on_scroll_up(x, y) {
                if !self.scrolling {
                    self.scrolling = true;
                    let room_id = self
                        .chat
                        .room
                        .current_room
                        .borrow()
                        .as_ref()
                        .unwrap()
                        .clone();

                    if let Err(e) = self.send_jobs.send(UserRequest::RoomMsgs(room_id)).await {
                        self.set_error(anyhow::Error::from(e))
                    }
                }
            } else {
                if self.chat.room.on_scroll_up(x, y) {
                    self.chat.msgs.reset_scroll()
                }
            }
        }
    }

    pub async fn on_scroll_down(&mut self, x: u16, y: u16) {
        if self.chat.main_screen {
            self.chat.msgs.on_scroll_down(x, y);
            // TODO make each widget's scroll method more similar
            if self.chat.room.on_scroll_down(x, y) {
                self.chat.msgs.reset_scroll()
            }
        }
    }

    pub async fn on_up(&mut self) {
        if !self.login_w.logged_in {
            if let LoginSelect::Username = self.login_w.login.selected {
                self.login_w.login.selected = LoginSelect::Password;
            } else {
                self.login_w.login.selected = LoginSelect::Username;
            }
        } else if self.chat.main_screen {
            self.chat.room.select_previous();
            self.chat.msgs.reset_scroll()
        }
    }

    pub async fn on_down(&mut self) {
        if !self.login_w.logged_in {
            if let LoginSelect::Username = self.login_w.login.selected {
                self.login_w.login.selected = LoginSelect::Password;
            } else {
                self.login_w.login.selected = LoginSelect::Username;
            }
        } else if self.chat.main_screen {
            self.chat.room.select_next();
            self.chat.msgs.reset_scroll()
        }
    }

    pub fn on_right(&mut self) {}

    pub fn on_left(&mut self) {}

    async fn add_char(&mut self, c: char) {
        if self.error.is_none() {
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
                    } else {
                        self.login_w.clear_login();
                    }
                }
                if let LoginSelect::Username = self.login_w.login.selected {
                    self.login_w.login.username.push(c);
                } else {
                    self.login_w.login.password.push(c);
                }
            } else if self.chat.main_screen {
                // send typing notice to the server
                let room_id = {
                    let id = self.chat.current_room.borrow();
                    id.deref().clone()
                };
                if !self.typing_notice {
                    self.typing_notice = true;
                    if let (Some(me), Some(room_id)) = (self.chat.me.clone(), room_id) {
                        if let Err(e) = self.send_jobs.send(UserRequest::Typing(room_id, me)).await
                        {
                            self.set_error(Error::from(e));
                        }
                    }
                }

                self.chat.msgs.add_char(c);
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
        } else if self.chat.main_screen {
            self.chat.msgs.pop();
        }
    }

    pub async fn on_delete(&mut self) {
        if self.chat.main_screen {
            let id = self.chat.current_room.borrow().deref().clone();
            if let Some(room_id) = id {
                if let Err(e) = self.send_jobs.send(UserRequest::LeaveRoom(room_id)).await {
                    self.set_error(anyhow::Error::from(e))
                } else {
                    self.chat.leaving_room = true;
                }
            }
        }
    }

    pub async fn on_send(&mut self) {
        // unfortunately we have to do it this way or we have a mutable borrow in the scope of immutable
        let res = if let Some(room_id) = self.chat.current_room.borrow().as_ref() {
            match self.chat.msgs.get_sending_message() {
                Ok(msg) => {
                    self.chat.sending_message = true;
                    let uuid = Uuid::new_v4();
                    let message = msg.clone();
                    if let Err(e) = self
                        .send_jobs
                        .send(UserRequest::SendMessage(room_id.clone(), msg, uuid))
                        .await
                    {
                        Err(anyhow::Error::from(e))
                    } else {
                        if let Some(room) = self.chat.room.rooms.get(room_id) {
                            let r = room.read().await;
                            let matrix_sdk::Room { members, .. } = r.deref();
                            let name = if let Some(mem) =
                                members.get(self.chat.msgs.me.as_ref().unwrap())
                            {
                                mem.name.clone()
                            } else {
                                self.chat.msgs.me.as_ref().unwrap().localpart().into()
                            };
                            self.chat.msgs.echo_sent_msg(
                                room_id,
                                name,
                                &self.homeserver,
                                uuid,
                                message,
                            );
                        }
                        self.chat.msgs.clear_send_msg();
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
            self.ev_loop.start_sync();
        }
        // this will login, send messages, and any other user initiated requests
        match self.ev_msgs.try_recv() {
            Ok(res) => match res {
                RequestResult::Login(res) => match res {
                    Err(e) => {
                        self.login_w.logging_in = false;
                        self.set_error(e);
                    }
                    Ok((rooms, resp)) => {
                        self.login_w.logged_in = true;
                        self.chat.main_screen = true;
                        self.login_w.logging_in = false;
                        self.chat.msgs.me = Some(resp.user_id.clone());
                        self.chat.me = Some(resp.user_id.clone());
                        self.chat.set_room_state(rooms).await;
                    }
                },
                // TODO this has the EventId which we need to keep
                RequestResult::SendMessage(res) => match res {
                    Err(e) => self.set_error(e),
                    Ok(_res) => self.chat.sending_message = false,
                },
                RequestResult::RoomMsgs(res) => match res {
                    Err(e) => {
                        // TODO recover from requesting room not currently joined
                        // this should be fixed once MembershipState::Leave is working
                        self.set_error(e)
                    }
                    Ok((res, room)) => {
                        self.process_room_events(res, room).await;
                        self.scrolling = false
                    }
                },
                RequestResult::AcceptInvite(res) => match res {
                    Err(e) => self.set_error(e),
                    Ok(res) => {
                        self.chat.joining_room = false;
                        if let Err(e) = self
                            .send_jobs
                            .send(UserRequest::RoomMsgs(res.room_id))
                            .await
                        {
                            self.set_error(anyhow::Error::from(e))
                        }
                    }
                },
                RequestResult::DeclineInvite(res, room_id) => {
                    if let Err(e) = res {
                        self.set_error(e);
                    }
                    self.chat.room.remove_room(room_id)
                }
                RequestResult::LeaveRoom(res, room_id) => {
                    if let Err(e) = res {
                        self.set_error(e);
                    }
                    self.chat.leaving_room = false;
                    self.chat.room.remove_room(room_id)
                }
                RequestResult::Typing(res) => {
                    if let Err(e) = res {
                        self.set_error(e);
                    }
                    self.typing_notice = false;
                }
                RequestResult::ReadReceipt(res) => {
                    if let Err(e) = res {
                        self.set_error(e);
                    }
                }
                // sync error
                RequestResult::Error(err) => self.set_error(err),
            },
            _ => {}
        }

        match self.emitter_msgs.try_recv() {
            Ok(res) => match res {
                // TODO make the MembershipState::Leave events work
                StateResult::Member {
                    sender,
                    receiver,
                    room,
                    membership,
                    timeline_event,
                } => {
                    let invitation = if let MembershipChange::Invited = membership {
                        true
                    } else {
                        false
                    };
                    // only display notifications for the current room
                    if self.chat.current_room.borrow().as_ref() == Some(&room.read().await.room_id)
                        // unless this is an invitation
                        || invitation
                        // or it is an event directed towards the user
                        // TODO in this case we should probably specify the room the event is for
                        || Some(&receiver) == self.chat.msgs.me.as_ref()
                    {
                        match membership {
                            MembershipChange::ProfileChanged => self.chat.msgs.add_notify(
                                &format!("{} updated their profile", receiver.localpart()),
                            ),
                            MembershipChange::Joined => {
                                if Some(&receiver) == self.chat.msgs.me.as_ref() {
                                    *self.chat.current_room.borrow_mut() =
                                        Some(room.read().await.room_id.clone());
                                    self.chat.room.add_room(room).await;
                                } else {
                                    self.chat.msgs.add_notify(&format!(
                                        "{} joined the room",
                                        sender.localpart()
                                    ));
                                }
                            }
                            MembershipChange::Invited => {
                                if Some(&receiver) == self.chat.msgs.me.as_ref() {
                                    // if this is a RoomEvent from the joined rooms timeline it is not
                                    // an actual invitation
                                    if !timeline_event {
                                        self.chat.room.invited(sender, room).await;
                                    }
                                } else {
                                    self.chat.msgs.add_notify(&format!(
                                        "{} was invited to the room",
                                        receiver.localpart()
                                    ));
                                }
                            }
                            MembershipChange::InvitationRejected => {
                                if Some(&receiver) == self.chat.msgs.me.as_ref() {
                                    self.chat.msgs.add_notify("you rejected an invitation");
                                    self.chat
                                        .room
                                        .remove_room(room.read().await.room_id.clone())
                                } else {
                                    self.chat.msgs.add_notify(&format!(
                                        "{} rejected an invitation to the room",
                                        receiver.localpart()
                                    ))
                                }
                            }
                            MembershipChange::InvitationRevoked => {
                                if Some(&receiver) == self.chat.msgs.me.as_ref() {
                                    self.chat.msgs.add_notify(&format!(
                                        "your invitation was rejected by {}",
                                        sender.localpart()
                                    ));
                                    self.chat
                                        .room
                                        .remove_room(room.read().await.room_id.clone())
                                } else {
                                    self.chat.msgs.add_notify(&format!(
                                        "{}'s invitations was rejected by {}",
                                        receiver.localpart(),
                                        sender.localpart(),
                                    ))
                                }
                            }
                            MembershipChange::Left => {
                                if Some(&receiver) == self.chat.msgs.me.as_ref() {
                                    self.chat.msgs.add_notify("you left the room");
                                    self.chat
                                        .room
                                        .remove_room(room.read().await.room_id.clone());
                                } else {
                                    self.chat.msgs.add_notify(&format!(
                                        "{} left the room",
                                        receiver.localpart()
                                    ))
                                }
                            }
                            MembershipChange::Banned => {
                                if Some(&receiver) == self.chat.msgs.me.as_ref() {
                                    self.chat.msgs.add_notify("you were banned from the room");
                                    self.chat
                                        .room
                                        .remove_room(room.read().await.room_id.clone())
                                } else {
                                    self.chat.msgs.add_notify(&format!(
                                        "{} was banned from the room",
                                        receiver.localpart()
                                    ))
                                }
                            }
                            MembershipChange::Unbanned => {
                                if Some(&receiver) == self.chat.msgs.me.as_ref() {
                                    self.chat.msgs.add_notify("you were unbanned from the room");
                                    self.chat
                                        .room
                                        .remove_room(room.read().await.room_id.clone())
                                } else {
                                    self.chat.msgs.add_notify(&format!(
                                        "{} was unbanned from the room",
                                        receiver.localpart()
                                    ))
                                }
                            }
                            MembershipChange::Kicked => {
                                if Some(&receiver) == self.chat.msgs.me.as_ref() {
                                    self.chat.msgs.add_notify("you were kicked from the room");
                                    self.chat
                                        .room
                                        .remove_room(room.read().await.room_id.clone())
                                } else {
                                    self.chat.msgs.add_notify(&format!(
                                        "{} was kicked from the room",
                                        receiver.localpart()
                                    ))
                                }
                            }
                            MembershipChange::KickedAndBanned => {
                                if Some(&receiver) == self.chat.msgs.me.as_ref() {
                                    self.chat
                                        .msgs
                                        .add_notify("you were kicked and banned from the room");
                                    self.chat
                                        .room
                                        .remove_room(room.read().await.room_id.clone())
                                } else {
                                    self.chat.msgs.add_notify(&format!(
                                        "{} was kicked and banned from the room",
                                        receiver.localpart()
                                    ))
                                }
                            }
                            MembershipChange::None => {}
                            MembershipChange::Error => panic!("membership error"),
                            _ => panic!("MembershipChange::NotImplemented is never valid bug"),
                        }
                    }
                }
                StateResult::Name(name, room_id) => self.chat.room.update_room(name, room_id),
                StateResult::Message(msg, room) => {
                    self.chat.msgs.add_message(msg, room);
                    if let Some((event, room)) = self.chat.msgs.read_receipt(self.last_interaction)
                    {
                        if let Err(e) = self
                            .send_jobs
                            .send(UserRequest::ReadReceipt(room, event))
                            .await
                        {
                            self.set_error(Error::from(e));
                        }
                    }
                }
                StateResult::MessageEdit(msg, room_id, event_id) => {
                    self.chat.msgs.edit_message(&room_id, &event_id, msg);
                }
                StateResult::FullyRead(event_id, room_id) => {
                    if self.chat.msgs.read_to_end(&event_id)
                        && self.chat.current_room.borrow().as_ref() == Some(&room_id)
                    {
                        // TODO
                        self.chat
                            .msgs
                            .add_notify("what is a read to end event? TODO")
                    }
                }
                StateResult::Typing(room_id, msg) => {
                    if self.chat.current_room.borrow().as_ref() == Some(&room_id) {
                        self.chat.msgs.add_notify(&msg)
                    }
                }
                StateResult::ReadReceipt(room_id, events) => {
                    let mut notices = vec![];
                    if self.chat.current_room.borrow().as_ref() == Some(&room_id) {
                        for e_id in self.chat.msgs.last_3_msg_event_ids() {
                            if let Some(rec) = events.get(e_id) {
                                if let Some(map) = &rec.read {
                                    // TODO keep track so we don't emit duplicate notices for
                                    // the same user with different EventIds
                                    for (user, receipt) in map {
                                        if receipt
                                            .ts
                                            .and_then(|ts| ts.elapsed().ok())
                                            // only show read receipts for the last 10 minutes
                                            .map(|dur| dur.as_secs() < 600)
                                            == Some(true)
                                        {
                                            notices.push(format!(
                                                "{} has seen the latest messages",
                                                user.localpart()
                                            ));
                                        }
                                    }
                                }
                            }
                        }
                    }
                    for notice in notices {
                        self.chat.msgs.add_notify(&notice);
                    }
                }
                StateResult::Reaction(_room_id, _event_id, _msg) => {}
                _ => {}
            },
            _ => {}
        }
    }

    pub async fn on_quit(&mut self) {
        self.ev_loop.quit_sync();
        if self.send_jobs.send(UserRequest::Quit).await.is_err() {
            // TODO what should happen when a send fails
            return;
        };
    }

    pub async fn on_notifications(&mut self) {
        let room_id = self.chat.current_room.borrow().deref().clone();
        if let Some(id) = room_id {
            let err = if let Some(room) = self.chat.room.rooms.get(&id) {
                let room = room.read().await;
                if let Some(event_id) = self.chat.msgs.check_unread(room.deref()) {
                    self.send_jobs
                        .send(UserRequest::ReadReceipt(id.clone(), event_id))
                        .await
                } else {
                    Ok(())
                }
            } else {
                Ok(())
            };

            if let Err(e) = err {
                self.set_error(anyhow::Error::from(e));
            }
        }
    }

    async fn process_room_events(
        &mut self,
        events: get_message_events::Response,
        room: Arc<RwLock<Room>>,
    ) {
        for ev in events.chunk {
            if let Ok(e) = ev.deserialize() {
                // matrix-sdk does not mutate the room on past events
                // rooms are only mutated for present events, so we must handle the past
                // events so when saved to the database the room is accurate with the current state
                room.write().await.receive_timeline_event(&e);

                match e {
                    RoomEvent::RoomMessage(msg) => {
                        let MessageEvent {
                            content,
                            sender,
                            event_id,
                            origin_server_ts,
                            unsigned,
                            ..
                        } = msg;

                        let name = {
                            let m = room.read().await;
                            m.members
                                .get(&sender)
                                .map(|m| m.name.to_string())
                                .unwrap_or(sender.localpart().to_string())
                        };

                        match content {
                            MessageEventContent::Text(TextMessageEventContent {
                                body: msg_body,
                                formatted_body,
                                ..
                            }) => {
                                let msg = if formatted_body.is_some() {
                                    crate::widgets::utils::markdown_to_terminal(&msg_body)
                                        .unwrap_or(msg_body.clone())
                                } else {
                                    msg_body.clone()
                                };
                                let txn_id = unsigned
                                    .transaction_id
                                    .as_ref()
                                    .map(|id| id.clone())
                                    .unwrap_or_default();

                                let msg = Message {
                                    name,
                                    user: sender.clone(),
                                    text: msg,
                                    event_id: event_id.clone(),
                                    timestamp: origin_server_ts,
                                    uuid: Uuid::parse_str(&txn_id).unwrap_or(Uuid::new_v4()),
                                    read: false,
                                    sent_receipt: false,
                                };
                                self.chat
                                    .msgs
                                    .add_message(msg, room.read().await.room_id.clone())
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

impl DrawWidget for AppWidget {
    fn draw<B: Backend + Send>(&mut self, terminal: &mut Terminal<B>) -> io::Result<()> {
        terminal.draw(|mut f| {
            let chunks = Layout::default()
                .constraints([Constraint::Length(3), Constraint::Min(0)].as_ref())
                .split(f.size());

            let text = if self.scrolling {
                vec![Text::styled(
                    "Loading previous messages",
                    Style::new().fg(Color::Green),
                )]
            } else if !self.login_w.logged_in {
                vec![Text::styled(
                    "Login to a Matrix Server",
                    Style::new().fg(Color::Green),
                )]
            } else if self.chat.joining_room {
                vec![Text::styled("Joining room", Style::new().fg(Color::Green))]
            } else if self.chat.leaving_room {
                vec![Text::styled("Leaving room", Style::new().fg(Color::Green))]
            } else if self.chat.sending_message {
                vec![Text::styled(
                    "Sending message",
                    Style::new().fg(Color::Green),
                )]
            } else if self.chat.main_screen {
                vec![Text::styled("Chatting", Style::new().fg(Color::Green))]
            } else {
                vec![Text::styled("", Style::new().fg(Color::Green))]
            };
            let para = Paragraph::new(text.iter())
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Green).modifier(Modifier::BOLD))
                        .title(&self.title)
                        .title_style(Style::default().fg(Color::Yellow).modifier(Modifier::BOLD)),
                )
                .alignment(Alignment::Center);

            f.render_widget(para, chunks[0]);

            let chunks2 = Layout::default()
                .constraints([Constraint::Percentage(100)].as_ref())
                .split(chunks[1]);

            if let Some(err) = self.error.as_ref() {
                ErrorWidget::new(err).render(&mut f, chunks2[0])
            } else if !self.login_w.logged_in {
                if self.login_w.homeserver.is_none() {
                    let domain = url::Url::parse(&self.homeserver)
                        .ok()
                        .and_then(|url| url.domain().map(|s| s.to_string()))
                        // this is probably an error at this point
                        .unwrap_or(String::from("matrix.org"));
                    self.login_w.homeserver = Some(domain);
                }
                self.login_w.render(&mut f, chunks2[0])
            } else {
                self.chat.render(&mut f, chunks2[0])
            }
        })
    }
}
