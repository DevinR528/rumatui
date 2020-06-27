use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, VecDeque},
    convert::TryFrom,
    fmt,
    ops::Deref,
    rc::Rc,
    sync::Arc,
    time::{Duration, SystemTime},
};

use js_int::UInt;
use matrix_sdk::events::{
    room::message::{
        FormattedBody, MessageEventContent, MessageFormat, RelatesTo, TextMessageEventContent,
    },
    AnyMessageEventStub, MessageEventStub,
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
    error::{Error, Result},
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
#[derive(Clone, Debug, Ord, PartialOrd, Eq, PartialEq)]
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
    messages: HashMap<RoomId, Vec<Message>>,
    pub(crate) me: Option<UserId>,
    pub unread_notifications: UInt,
    send_msgs: HashMap<RoomId, String>,
    notifications: VecDeque<(Option<SystemTime>, String)>,
    scroll_pos: usize,
    did_overflow: Option<Rc<Cell<bool>>>,
    at_top: Option<Rc<Cell<bool>>>,
}

impl MessageWidget {
    pub async fn populate_initial_msgs(&mut self, rooms: &HashMap<RoomId, Arc<RwLock<Room>>>) {
        for room in rooms.values() {
            let room = room.read().await;

            self.send_msgs.insert(room.room_id.clone(), String::new());

            self.unread_notifications = room.unread_notifications.unwrap_or_default();
            self.unread_notifications += room.unread_highlight.unwrap_or_default();

            // TODO handle redactions
            for msg in room.messages.iter() {
                match &msg.deref() {
                    AnyMessageEventStub::RoomMessage(event) => self.add_message_event(event, &room),
                    _ => {}
                }
            }
        }
    }

    pub async fn add_room(&mut self, room: Arc<RwLock<Room>>) {
        self.send_msgs
            .insert(room.read().await.room_id.clone(), String::new());
    }

    // TODO factor out with AppWidget::process_room_events and MessageWidget::echo_sent_msg
    fn add_message_event(&mut self, event: &MessageEventStub<MessageEventContent>, room: &Room) {
        let MessageEventStub {
            content,
            sender,
            event_id,
            origin_server_ts,
            unsigned,
            ..
        } = event;
        let name = if let Some(mem) = room.joined_members.get(&sender) {
            mem.name()
        } else {
            sender.localpart().into()
        };
        match content {
            MessageEventContent::Text(TextMessageEventContent {
                body, formatted, ..
            }) => {
                let msg = if formatted
                    .as_ref()
                    .map(|f| f.body.to_string())
                    .unwrap_or(String::new())
                    != body.to_string()
                {
                    crate::widgets::utils::markdown_to_terminal(body).unwrap_or(body.clone())
                // None.unwrap_or(body.clone())
                } else {
                    body.clone()
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
        if let Some(messages) = self.messages.get_mut(room) {
            // remove the message echo when user sends a message and we display the text before
            // the server responds
            if let Some(idx) = messages.iter().position(|m| m.uuid == msg.uuid) {
                messages[idx] = msg;
                return;
            }
        }
        self.messages.entry(room.clone()).or_default().push(msg);
        // TODO scroll seems to keep up but keep an eye on it
        // self.calculate_scroll_down();
    }

    pub fn edit_message(&mut self, room: &RoomId, event_id: &EventId, msg: String) {
        if let Some(messages) = self.messages.get_mut(room) {
            // remove the message echo when user sends a message and we display the text before
            // the server responds
            if let Some(idx) = messages.iter().position(|m| &m.event_id == event_id) {
                messages[idx].text = msg;
            }
        }
    }

    pub fn add_notify(&mut self, notify: &str) {
        self.notifications.push_back((None, notify.to_string()));
    }

    pub fn set_reaction_event(
        &mut self,
        room: &RoomId,
        relates_to: &EventId,
        event_id: &EventId,
        reaction: &str,
    ) {
        if let Some(messages) = self.messages.get_mut(room) {
            if let Some(idx) = messages.iter().position(|m| &m.event_id == relates_to) {
                messages[idx].reactions.push(Reaction {
                    key: reaction.to_string(),
                    event_id: event_id.clone(),
                });
            }
        }
    }

    pub fn redaction_event(&mut self, room: &RoomId, event_id: &EventId) {
        if let Some(messages) = self.messages.get_mut(room) {
            for message in messages {
                if &message.event_id == event_id {
                    message.text = "**R**E**D**A**C**T**E**D**".to_string();
                }
                // TODO PR rust for better docs on `.retain()` method yee...
                message
                    .reactions
                    .retain(|emoji| &emoji.event_id != event_id);
            }
        }
    }

    pub fn clear_send_msg(&mut self) {
        if let Some(room_id) = self.current_room.borrow().deref() {
            if let Some(msg) = self.send_msgs.get_mut(room_id) {
                msg.clear()
            }
        }
    }

    // TODO Im sure there is an actual way to do this like Riot
    // TODO fix message text box hashmap
    fn process_message(&self) -> Result<MsgType> {
        if let Some(room_id) = self.current_room.borrow().deref() {
            if let Some(msg) = self.send_msgs.get(room_id) {
                if msg.contains('`') {
                    Ok(MsgType::FormattedText)
                } else {
                    Ok(MsgType::PlainText)
                }
            } else {
                Err(Error::Rumatui(
                    "The room was added to the send_msgs HashMap rumatui BUG",
                ))
            }
        } else {
            Err(Error::Rumatui("No current room has been set rumatui BUG"))
        }
    }

    // TODO fix message text box hashmap
    pub fn get_sending_message(&self) -> Result<MessageEventContent> {
        if let Some(room_id) = self.current_room.borrow().deref() {
            if let Some(to_send) = self.send_msgs.get(room_id) {
                match self.process_message()? {
                    MsgType::PlainText => Ok(MessageEventContent::Text(
                        TextMessageEventContent::new_plain(to_send.as_str()),
                    )),
                    MsgType::FormattedText => {
                        Ok(MessageEventContent::Text(TextMessageEventContent {
                            body: to_send.to_string(),
                            formatted: Some(FormattedBody {
                                format: MessageFormat::Html,
                                body: markdown_to_html(&to_send),
                            }),
                            relates_to: None::<RelatesTo>,
                        }))
                    }
                    _ => todo!("implement more sending messages"),
                }
            } else {
                Err(Error::Rumatui(
                    "The room was added to the send_msgs HashMap rumatui BUG",
                ))
            }
        } else {
            Err(Error::Rumatui("No current room has been set rumatui BUG"))
        }
    }

    pub fn echo_sent_msg(
        &mut self,
        id: &RoomId,
        name: String,
        uuid: Uuid,
        content: MessageEventContent,
    ) {
        match content {
            MessageEventContent::Text(TextMessageEventContent {
                body, formatted, ..
            }) => {
                let msg = if formatted
                    .as_ref()
                    .map(|f| f.body.to_string())
                    .unwrap_or(String::new())
                    != body.to_string()
                {
                    crate::widgets::utils::markdown_to_terminal(&body).unwrap_or(body.clone())
                // None.unwrap_or(body.clone())
                } else {
                    body
                };
                let timestamp = SystemTime::now();

                let msg = Message {
                    text: msg,
                    user: self.me.as_ref().unwrap().clone(),
                    timestamp,
                    name,
                    event_id: EventId::try_from("$fake:rumatui.event").unwrap(),
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

    pub(crate) fn read_to_end(&self, room: &RoomId, event_id: &EventId) -> bool {
        if let Some(messages) = self.messages.get(room) {
            messages.last().map(|msg| &msg.event_id) == Some(event_id)
        } else {
            false
        }
    }

    pub(crate) fn last_3_msg_event_ids(&self, room: &RoomId) -> Vec<&EventId> {
        if let Some(messages) = self.messages.get(room) {
            messages[self.messages.len() - 4..]
                .iter()
                .map(|msg| &msg.event_id)
                .collect()
        } else {
            vec![]
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

    // TODO remove this or `check_unread` eventually
    pub(crate) fn read_receipt(
        &mut self,
        last_interaction: SystemTime,
        room_id: &RoomId,
    ) -> Option<EventId> {
        if last_interaction.elapsed().ok()? < Duration::from_secs(2) {
            if let Some(messages) = self.messages.get_mut(room_id) {
                messages.sort_by(|msg, msg2| msg.timestamp.cmp(&msg2.timestamp));

                for msg in messages.iter_mut().rev() {
                    // if the message is older than 3 days give up
                    if msg.timestamp.elapsed().ok()? > Duration::from_secs(259200) {
                        return None;
                    }

                    if msg.read && !msg.sent_receipt {
                        msg.sent_receipt = true;
                        return Some(msg.event_id.clone());
                    }
                }
                None
            } else {
                // this is possibly a larger problem as we are looking in a room we aren't joined?
                None
            }
        } else {
            None
        }
    }

    pub fn check_unread(&mut self, room: &Room) -> Option<EventId> {
        self.unread_notifications = room.unread_notifications.unwrap_or_default();

        self.unread_notifications += room.unread_highlight.unwrap_or_default();

        if let Some(messages) = self.messages.get_mut(&room.room_id) {
            messages.sort_by(|msg, msg2| msg.timestamp.cmp(&msg2.timestamp));

            for msg in messages.iter_mut().rev() {
                // if the message is older than 1.5 days give up
                if msg.timestamp.elapsed().ok()? > Duration::from_secs(86400) {
                    return None;
                }

                if msg.read && !msg.sent_receipt {
                    msg.sent_receipt = true;
                    return Some(msg.event_id.clone());
                }
            }
            None
        } else {
            None
        }
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

    // TODO fix message text box hashmap
    pub fn add_char(&mut self, ch: char) {
        self.send_msgs
            .get_mut(self.current_room.borrow().as_ref().unwrap())
            .map(|m| m.push(ch));
    }

    // TODO fix message text box hashmap
    pub fn remove_char(&mut self) {
        self.send_msgs
            .get_mut(self.current_room.borrow().as_ref().unwrap())
            .map(|m| m.pop());
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

        // TODO handle getting the textbox message better
        let sending_text = if let Some(room_id) = self.current_room.borrow().as_ref() {
            self.send_msgs
                .get(room_id)
                // TODO
                .cloned()
                .unwrap_or_default()
        } else {
            String::new()
        };

        let mut lines = sending_text.chars().filter(|c| *c == '\n').count();
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
            Some(id.clone())
        } else {
            // or take the first room in the list, this happens on login
            self.messages.keys().next().cloned()
        };

        let mut msg_copy = vec![];
        if let Some(room_id) = current_room_id {
            if let Some(messages) = self.messages.get_mut(&room_id) {
                messages.sort_by(|msg, msg2| msg.timestamp.cmp(&msg2.timestamp));
                // make sure the messages we have seen are marked read.
                for mark_msg in messages.iter_mut().rev().take(5) {
                    // this message has been read and a read receipt will be sent for it
                    mark_msg.read = true;
                }
                for msg in messages
                    .iter_mut()
                    .unique_by(|msg| msg.event_id.clone())
                    .flat_map(|msg| ctrl_char::process_text(msg))
                {
                    msg_copy.push(msg);
                }
            }
        }

        let (title, style) = if self.unread_notifications > UInt::MIN {
            (
                format!(
                    "-----Messages-----unread {}",
                    self.unread_notifications.to_string()
                ),
                Style::default().fg(Color::Red).modifier(Modifier::BOLD),
            )
        } else {
            (
                "-----Messages-----".to_string(),
                Style::default().fg(Color::Yellow).modifier(Modifier::BOLD),
            )
        };
        let messages = Paragraph::new(msg_copy.iter())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Green).modifier(Modifier::BOLD))
                    .title(&title)
                    .title_style(style),
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
            Text::styled(&sending_text, Style::default().fg(Color::Blue)),
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
