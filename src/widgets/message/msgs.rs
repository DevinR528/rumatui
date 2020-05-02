use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use itertools::Itertools;
use matrix_sdk::events::room::message::{
    MessageEvent, MessageEventContent, TextMessageEventContent,
};
use matrix_sdk::identifiers::{EventId, RoomId, UserId};
use matrix_sdk::Room;
use termion::event::MouseButton;
use tokio::sync::RwLock;
use tui::backend::Backend;
use tui::layout::{Constraint, Direction, Layout, Rect, ScrollMode};
use tui::style::{Color, Modifier, Style};
use tui::widgets::{Block, Borders, Paragraph, Text};
use tui::Frame;
use uuid::Uuid;

use crate::widgets::{message::ctrl_char, utils::markdown_to_html, RenderWidget};

/// A wrapper to abstract a `RoomEvent::RoomMessage` and the MessageEvent queue
/// from `matrix_sdk::Room`.
#[derive(Clone, Debug)]
pub struct Message {
    pub name: String,
    pub text: String,
    pub user: UserId,
    pub event_id: EventId,
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

// TODO split MessageWidget into render and state halves. MsgRender has the methods to filter
// and populate messages using the messages Vec. MessageState or Data?? will populate and keep track of
// state, add_message_event, get_sending_message, process and such are part of state/data

#[derive(Clone, Debug, Default)]
pub struct MessageWidget {
    msg_area: Rect,
    send_area: Rect,
    // TODO save this to a local "database" somehow
    /// This is the RoomId of the last used room.
    pub(crate) current_room: Rc<RefCell<Option<RoomId>>>,
    messages: Vec<(RoomId, Message)>,
    pub(crate) me: Option<UserId>,
    send_msg: String,
    notifications: VecDeque<(SystemTime, String)>,
    scroll_pos: usize,
    did_overflow: Option<Rc<Cell<bool>>>,
    at_top: Option<Rc<Cell<bool>>>,
}

impl MessageWidget {
    pub async fn populate_initial_msgs(&mut self, rooms: &HashMap<RoomId, Arc<RwLock<Room>>>) {
        for (_id, room) in rooms {
            let room = room.read().await;
            for msg in room.messages.iter() {
                self.add_message_event(msg, &room);
            }
        }
    }

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
                    .get("transaction_id")
                    .map(|id| serde_json::from_value::<String>(id.clone()).unwrap())
                    .unwrap_or_default();

                self.add_message(
                    Message {
                        name,
                        user: sender.clone(),
                        text: msg,
                        event_id: event_id.clone(),
                        timestamp: *origin_server_ts,
                        uuid: Uuid::parse_str(&txn_id).unwrap_or(Uuid::new_v4()),
                    },
                    room.room_id.clone(),
                );
            }
            _ => {}
        }
    }

    pub fn add_message(&mut self, msg: Message, room: RoomId) {
        // remove the message echo when user sends a message and we display the text before
        // the server responds
        if let Some(idx) = self.messages.iter().position(|(_, m)| m.uuid == msg.uuid) {
            self.messages[idx] = (room, msg);
            return;
        }
        self.messages.push((room, msg));
        // self.calculate_scroll_down();
    }

    pub fn add_notify(&mut self, notify: &str) {
        self.notifications
            .push_back((SystemTime::now(), notify.to_string()));
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

    pub fn get_sending_message(&self) -> Result<MessageEventContent, anyhow::Error> {
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
                    body.clone()
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
                };
                self.add_message(msg, id.clone())
            }
            _ => {}
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

    pub fn pop(&mut self) {
        self.send_msg.pop();
    }
}

impl RenderWidget for MessageWidget {
    fn render<B: Backend>(&mut self, f: &mut Frame<B>, area: Rect) {
        if let None = self.did_overflow {
            self.did_overflow = Some(Rc::new(Cell::new(false)));
        }
        if let None = self.at_top {
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
            .map(|x| x)
            .flat_map(|(_, msg)| ctrl_char::process_text(msg))
            .collect::<Vec<_>>();

        let messages = Paragraph::new(text.iter())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Green).modifier(Modifier::BOLD))
                    .title("Messages")
                    .title_style(Style::default().fg(Color::Yellow).modifier(Modifier::BOLD)),
            )
            .wrap(true)
            .scroll(self.scroll_pos as u16)
            .scroll_mode(ScrollMode::Tail)
            .did_overflow(Rc::clone(self.did_overflow.as_ref().unwrap()))
            .at_top(Rc::clone(self.at_top.as_ref().unwrap()));

        f.render_widget(messages, chunks[0]);

        // display each notification for 3 seconds
        if let Some((time, _item)) = self.notifications.get(0) {
            if let Ok(elapsed) = time.elapsed() {
                if elapsed > Duration::from_secs(8) {
                    let _ = self.notifications.pop_front();
                }
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
