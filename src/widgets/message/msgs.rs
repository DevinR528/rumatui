use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use js_int::UInt;
use matrix_sdk::events::room::message::{
    MessageEvent, MessageEventContent, TextMessageEventContent,
};
use matrix_sdk::{
    identifiers::{EventId, RoomId, UserId},
    Room,
};
use rumatui_tui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect, ScrollMode},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph, Text},
    Frame,
};
use termion::event::MouseButton;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::{
    error::Result,
    widgets::{message::ctrl_char, utils::markdown_to_html, RenderWidget},
};

/// A reaction event containing the string (emoji) and the event id for the reaction
/// event not the event it relates to.
#[derive(Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
pub struct Reaction {
    pub key: String,
    pub event_id: EventId,
}

impl fmt::Display for Reaction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.key)
    }
}

/// A wrapper to abstract a `RoomEvent::RoomMessage` and the MessageEvent queue
/// from `matrix_sdk::Room`.
#[derive(Clone, Debug)]
pub struct Message {
    pub name: String,
    pub text: String,
    pub user: UserId,
    pub event_id: EventId,
    /// Has this `Message` been seen.
    ///
    /// This is true when the user is active and the message appears in the
    /// `MessageWidget` window.
    pub read: bool,
    /// A vector of all the reactions this "event/message" has received.
    pub reactions: Vec<Reaction>,
    /// Has the read_receipt been sent.
    pub sent_receipt: bool,
    pub timestamp: SystemTime,
    pub uuid: Uuid,
}

pub enum MsgType {
    PlainText,
    FormattedText,
    RichReply,
    Audio,
    Emote,
    File,
    Location,
    Image,
    ServerNotice,
    Video,
}

#[derive(Clone, Debug, Default)]
pub struct MessageWidget {
    msg_area: Rect,
    send_area: Rect,
    // TODO save this to a local "database" somehow
    /// This is the RoomId of the last used room.
    pub(crate) current_room: Rc<RefCell<Option<RoomId>>>,
    messages: Vec<(RoomId, Message)>,
    pub(crate) me: Option<UserId>,
    pub unread_notifications: UInt,
    send_msg: String,
    notifications: VecDeque<(Option<SystemTime>, String)>,
    scroll_pos: usize,
    did_overflow: Option<Rc<Cell<bool>>>,
    at_top: Option<Rc<Cell<bool>>>,
}

impl MessageWidget {
    pub async fn populate_initial_msgs(&mut self, rooms: &HashMap<RoomId, Arc<RwLock<Room>>>) {
        for room in rooms.values() {
            let room = room.read().await;
            self.unread_notifications = room.unread_notifications.unwrap_or_default();

            self.unread_notifications += room.unread_highlight.unwrap_or_default();

            for msg in room.messages.iter() {
                self.add_message_event(msg, &room);
            }
        }
    }

    // TODO factor out with AppWidget::process_room_events and MessageWidget::echo_sent_msg
    fn add_message_event(&mut self, event: &MessageEvent, room: &Room) {
        let MessageEvent {
            content,
            sender,
            event_id,
            origin_server_ts,
            unsigned,
            ..
        } = event;
        let name = if let Some(mem) = room.members.get(&sender) {
            mem.name.clone()
        } else {
            sender.localpart().into()
        };
        match content {
            MessageEventContent::Text(TextMessageEventContent {
                body: msg_body,
                formatted_body,
                ..
            }) => {
                let msg = if let Some(_fmted) = formatted_body {
                    crate::widgets::utils::markdown_to_terminal(msg_body)
                        .unwrap_or(msg_body.clone())
                } else {
                    msg_body.clone()
                };
                let txn_id = unsigned
                    .transaction_id
                    .as_ref()
                    .cloned()
                    .unwrap_or_default();

                self.add_message(
                    Message {
                        name,
                        user: sender.clone(),
                        text: msg,
                        event_id: event_id.clone(),
                        timestamp: *origin_server_ts,
                        uuid: Uuid::parse_str(&txn_id).unwrap_or(Uuid::new_v4()),
                        read: false,
                        reactions: vec![],
                        sent_receipt: false,
                    },
                    &room.room_id,
                );
            }
            _ => {}
        }
    }

    pub fn add_message(&mut self, msg: Message, room: &RoomId) {
        // remove the message echo when user sends a message and we display the text before
        // the server responds
        if let Some(idx) = self.messages.iter().position(|(_, m)| m.uuid == msg.uuid) {
            self.messages[idx] = (room.clone(), msg);
            return;
        }
        self.messages.push((room.clone(), msg));
        // self.calculate_scroll_down();
    }

    pub fn edit_message(&mut self, room: &RoomId, event_id: &EventId, msg: String) {
        // remove the message echo when user sends a message and we display the text before
        // the server responds
        if let Some(idx) = self
            .messages
            .iter()
            .position(|(id, m)| &m.event_id == event_id && room == id)
        {
            self.messages[idx].1.text = msg;
        }
    }

    pub fn add_notify(&mut self, notify: &str) {
        self.notifications.push_back((None, notify.to_string()));
    }

    pub fn set_reaction_event(&mut self, room: &RoomId, relates_to: &EventId, event_id: &EventId, reaction: &str) {
        if let Some(idx) = self
            .messages
            .iter()
            .position(|(id, m)| &m.event_id == relates_to && room == id)
        {
            self.messages[idx].1.reactions.push(Reaction { key: reaction.to_string(), event_id: event_id.clone(), });
        }
    }

    pub fn redaction_event(&mut self, room: &RoomId, event_id: &EventId) {
        for (id, message) in &mut self.messages {
            if &message.event_id == event_id && room == id {
                message.text = "**R**E**D**A**C**T**E**D**".to_string();
            }
            // TODO PR rust for better docs on `.retain()` method yee...
            message.reactions.retain(|emoji| &emoji.event_id != event_id);
        }
        if let Some(idx) = self
            .messages
            .iter()
            .position(|(id, m)| &m.event_id == event_id && room == id)
        {
            self.messages[idx].1.text = "**R**E**D**A**C**T**E**D**".to_string();
        }
    }

    pub fn clear_send_msg(&mut self) {
        self.send_msg.clear();
    }

    // TODO Im sure there is an actual way to do this like Riot or a matrix server
    fn process_message(&self) -> MsgType {
        if self.send_msg.contains('`') {
            MsgType::FormattedText
        } else {
            MsgType::PlainText
        }
    }

    pub fn get_sending_message(&self) -> Result<MessageEventContent> {
        match self.process_message() {
            MsgType::PlainText => Ok(MessageEventContent::Text(TextMessageEventContent {
                body: self.send_msg.clone(),
                format: None,
                formatted_body: None,
                relates_to: None,
            })),
            MsgType::FormattedText => Ok(MessageEventContent::Text(TextMessageEventContent {
                body: self.send_msg.clone(),
                format: Some("org.matrix.custom.html".into()),
                formatted_body: Some(markdown_to_html(&self.send_msg)),
                relates_to: None,
            })),
            _ => todo!("implement more sending messages"),
        }
    }

    pub fn echo_sent_msg(
        &mut self,
        id: &RoomId,
        name: String,
        homeserver: &str,
        uuid: Uuid,
        content: MessageEventContent,
    ) {
        match content {
            MessageEventContent::Text(TextMessageEventContent {
                body,
                formatted_body,
                ..
            }) => {
                let msg = if let Some(_fmted) = formatted_body {
                    crate::widgets::utils::markdown_to_terminal(&body).unwrap_or(body.clone())
                } else {
                    body
                };
                let timestamp = SystemTime::now();
                let domain = url::Url::parse(homeserver)
                    .ok()
                    .and_then(|url| url.domain().map(|s| s.to_string()))
                    // this is probably an error at this point
                    .unwrap_or(String::from("matrix.org"));
                let msg = Message {
                    text: msg,
                    user: self.me.as_ref().unwrap().clone(),
                    timestamp,
                    name,
                    event_id: EventId::new(&domain).unwrap(),
                    uuid,
                    read: true,
                    reactions: vec![],
                    sent_receipt: true,
                };
                self.add_message(msg, id)
            }
            _ => {}
        }
    }

    pub(crate) fn read_to_end(&self, event_id: &EventId) -> bool {
        self.messages.last().map(|(_, msg)| &msg.event_id) == Some(event_id)
    }

    pub(crate) fn last_3_msg_event_ids(&self) -> impl Iterator<Item = &EventId> {
        self.messages[self.messages.len() - 4..]
            .iter()
            .map(|(_, msg)| &msg.event_id)
    }

    pub(crate) fn read_receipt(
        &mut self,
        last_interaction: SystemTime,
    ) -> Option<(EventId, RoomId)> {
        if last_interaction.elapsed().ok()? < Duration::from_secs(60) {
            self.messages
                .iter()
                .find(|(_id, msg)| msg.read && !msg.sent_receipt)
                .map(|(id, msg)| (msg.event_id.clone(), id.clone()))
        } else {
            None
        }
    }

    pub fn on_click(&mut self, btn: MouseButton, x: u16, y: u16) -> bool {
        if self.send_area.intersects(Rect::new(x, y, 1, 1)) {
            if let MouseButton::Left = btn {
                return true;
            }
        }
        false
    }

    pub fn reset_scroll(&mut self) {
        self.scroll_pos = 0;
        if let Some(over) = self.did_overflow.as_ref() {
            over.set(false);
        }
        if let Some(top) = self.at_top.as_ref() {
            top.set(false);
        }
    }

    pub fn check_unread(&mut self, room: &Room) -> Option<EventId> {
        self.unread_notifications = room.unread_notifications.unwrap_or_default();

        self.unread_notifications += room.unread_highlight.unwrap_or_default();

        self.messages
            .iter()
            .rfind(|(_id, msg)| msg.read)
            .map(|(_, msg)| msg.event_id.clone())
    }

    pub fn on_scroll_up(&mut self, x: u16, y: u16) -> bool {
        let intersects = self.msg_area.intersects(Rect::new(x, y, 1, 1));
        if intersects {
            if let Some(overflow) = self.did_overflow.as_ref() {
                if overflow.get() {
                    if let Some(at_top) = self.at_top.as_ref() {
                        if at_top.get() {
                            at_top.set(false);
                            return true;
                        } else {
                            self.scroll_pos += 1;
                            return false;
                        }
                    } else {
                        self.scroll_pos += 1;
                        return false;
                    }
                } else {
                    return true;
                }
            } else {
                unreachable!("did_overflow was not set")
            }
        }
        false
    }

    fn calculate_scroll_down(&mut self) {
        if let Some(overflow) = self.did_overflow.as_ref() {
            if overflow.get() && self.scroll_pos != 0 {
                self.scroll_pos -= 1;
            }
        }
    }

    pub fn on_scroll_down(&mut self, x: u16, y: u16) {
        if self.msg_area.intersects(Rect::new(x, y, 1, 1)) {
            self.calculate_scroll_down();
        }
    }

    pub fn add_char(&mut self, ch: char) {
        self.send_msg.push(ch);
    }

    pub fn remove_char(&mut self) {
        self.send_msg.pop();
    }
}

impl RenderWidget for MessageWidget {
    fn render<B: Backend>(&mut self, f: &mut Frame<B>, area: Rect) {
        use itertools::Itertools;

        if self.did_overflow.is_none() {
            self.did_overflow = Some(Rc::new(Cell::new(false)));
        }
        if self.at_top.is_none() {
            self.at_top = Some(Rc::new(Cell::new(false)));
        }
        let mut lines = self.send_msg.chars().filter(|c| *c == '\n').count();
        if lines <= 1 {
            lines = 2;
        }
        let (msg_height, send_height, notify_height) = {
            let send = ((lines + 1) * 5) as u16;
            let notify = if area.height < 25 { 0 } else { 15 };
            (100 - (send + 15), send, notify)
        };
        let chunks = Layout::default()
            .constraints(
                [
                    Constraint::Percentage(msg_height),
                    Constraint::Percentage(notify_height),
                    Constraint::Percentage(send_height),
                ]
                .as_ref(),
            )
            .direction(Direction::Vertical)
            .split(area);

        self.msg_area = chunks[0];
        let b = self.current_room.borrow();
        let current_room_id = if let Some(id) = b.as_ref() {
            Some(id)
        } else {
            // or take the first room in the list, this happens on login
            self.messages.first().map(|(id, _msg)| id)
        };

        // TODO no alloc
        let mut messages = self.messages.clone();
        messages.sort_by(|(_, msg), (_, msg2)| msg.timestamp.cmp(&msg2.timestamp));
        let text = messages
            .iter()
            .filter(|(id, _)| Some(id) == current_room_id)
            .unique_by(|(_id, msg)| &msg.event_id)
            .flat_map(|(_, msg)| ctrl_char::process_text(msg))
            .collect::<Vec<_>>();

        // make sure the messages we have seen are marked read.
        if !text.is_empty() {
            for idx in 0..text.len() - 1 {
                if let Some((_, msg)) = self.messages.get_mut(idx) {
                    msg.read = true;
                }
            }
        }

        let title = format!("Messages       unread ==>{}", self.unread_notifications.to_string(),);
        let messages = Paragraph::new(text.iter())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Green).modifier(Modifier::BOLD))
                    .title(&title)
                    .title_style(Style::default().fg(Color::Yellow).modifier(Modifier::BOLD)),
            )
            .wrap(true)
            .scroll(self.scroll_pos as u16)
            .scroll_mode(ScrollMode::Tail)
            .did_overflow(Rc::clone(self.did_overflow.as_ref().unwrap()))
            .at_top(Rc::clone(self.at_top.as_ref().unwrap()));

        f.render_widget(messages, chunks[0]);

        // display each notification for 6 seconds
        if let Some((time, _item)) = self.notifications.get_mut(0) {
            if let Some(time) = time {
                if let Ok(elapsed) = time.elapsed() {
                    if elapsed > Duration::from_secs(6) {
                        let _ = self.notifications.pop_front();
                    }
                }
            } else {
                *time = Some(SystemTime::now());
            }
        }

        let t2 = vec![Text::styled(
            self.notifications
                .get(0)
                .map(|(_time, item)| item.as_str())
                .unwrap_or("Notifications..."),
            Style::default().fg(Color::Green),
        )];
        let notification = Paragraph::new(t2.iter())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Green).modifier(Modifier::BOLD))
                    .title_style(Style::default().fg(Color::Yellow).modifier(Modifier::BOLD)),
            )
            .wrap(true);

        f.render_widget(notification, chunks[1]);

        let t3 = vec![
            Text::styled(&self.send_msg, Style::default().fg(Color::Blue)),
            Text::styled(
                "<",
                Style::default()
                    .fg(Color::LightGreen)
                    .modifier(Modifier::RAPID_BLINK),
            ),
        ];
        let text_box = Paragraph::new(t3.iter())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Green).modifier(Modifier::BOLD))
                    .title("Send")
                    .title_style(Style::default().fg(Color::Yellow).modifier(Modifier::BOLD)),
            )
            .wrap(true);

        f.render_widget(text_box, chunks[2]);

        let btn = Layout::default()
            .constraints([Constraint::Percentage(90), Constraint::Percentage(10)].as_ref())
            .direction(Direction::Horizontal)
            .split(chunks[2]);

        self.send_area = btn[1];

        let btn_text = vec![Text::styled("Send", Style::default().fg(Color::Blue))];
        let button = Paragraph::new(btn_text.iter()).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Green).modifier(Modifier::BOLD))
                .title_style(Style::default().fg(Color::Yellow).modifier(Modifier::BOLD)),
        );
        f.render_widget(button, btn[1]);
    }
}
