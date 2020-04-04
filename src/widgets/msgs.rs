use std::cell::RefCell;
use std::rc::Rc;

use matrix_sdk::identifiers::RoomId;
use matrix_sdk::events::room::message::{
    MessageEventContent, TextMessageEventContent,
};
use tui::backend::{Backend};
use tui::layout::{Constraint, Direction, Layout, Rect};
use tui::style::{Color, Modifier, Style};
use tui::widgets::{Block, Borders, Paragraph, Text};
use tui::{Frame, Terminal};

use super::app::{RenderWidget};
use super::utils::write_markdown_string;


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
    area: Rect,
    // TODO save this to a local "database" somehow
    /// This is the RoomId of the last used room.
    pub(crate) current_room: Rc<RefCell<Option<RoomId>>>,
    messages: Vec<(RoomId, String)>,
    send_msg: String,
}

impl MessageWidget {
    pub fn add_message(&mut self, msg: String, room: RoomId) {
        self.messages.push((room, msg))
    }

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
                formatted_body: Some(write_markdown_string(&self.send_msg)?),
                relates_to: None,
            })),
            _ => todo!("implement more sending messages")
        }
    }

    pub fn add_char(&mut self, ch: char) -> bool {
        if ch == '\n' {
            self.messages.push((self.current_room.borrow().as_ref().unwrap().clone(), write_markdown_string(&self.send_msg).unwrap()));
            true
        } else {
            self.send_msg.push(ch);
            false
        }
    }
    pub fn pop(&mut self) {
        self.send_msg.pop();
    }
}

impl RenderWidget for MessageWidget {
    fn render<B: Backend>(&mut self, f: &mut Frame<B>, area: Rect) {
        self.area = area;
        let chunks = Layout::default()
            .constraints([Constraint::Percentage(90), Constraint::Percentage(10)].as_ref())
            .direction(Direction::Vertical)
            .split(area);

        let b = self.current_room.borrow();
        let cmp_id = if let Some(id) = b.as_ref() {
            Some(id)
        } else {
            self.messages.first().map(|(id, _msg)| id)
        };

        let text = self
            .messages
            .iter()
            .filter(|(id, _)| Some(id) == cmp_id)
            .map(|(_, msg)| msg.to_string())
            .collect::<Vec<_>>()
            .join("\n");

        let t = vec![Text::styled(text, Style::default().fg(Color::Blue))];
        let p = Paragraph::new(t.iter())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Green).modifier(Modifier::BOLD))
                    .title("Messages")
                    .title_style(Style::default().fg(Color::Yellow).modifier(Modifier::BOLD)),
            )
            .wrap(true);

        f.render_widget(p, chunks[0]);
        
        let t2 = vec![Text::styled(&self.send_msg, Style::default().fg(Color::Blue))];
        let p2 = Paragraph::new(t2.iter())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Green).modifier(Modifier::BOLD))
                    .title("Send")
                    .title_style(Style::default().fg(Color::Yellow).modifier(Modifier::BOLD)),
            )
            .wrap(true);
        
        f.render_widget(p2, chunks[1]);
    }
}

