use std::sync::{Arc, RwLock};
use std::cell::RefCell;
use std::rc::Rc;

use tui::backend::Backend;
use tui::layout::{Constraint, Direction, Layout, Rect};
use tui::style::{Color, Modifier, Style};
use tui::widgets::{Block, Borders, Paragraph, Text, Widget};
use tui::Frame;

use super::app::RenderWidget;

#[derive(Clone, Debug, Default)]
pub struct MessageWidget {
    area: Rect,
    /// This is the RoomId of the last used room, the room to show on startup.
    pub(crate) current_room: Rc<RefCell<Option<crate::RoomIdStr>>>,
    messages: Vec<(crate::RoomIdStr, String)>,
}

impl MessageWidget {
    pub fn add_message(&mut self, msg: String, room: crate::RoomIdStr) {
        self.messages.push((room, msg))
    }
}

impl RenderWidget for MessageWidget {
    fn render<B>(&mut self, f: &mut Frame<B>, area: Rect)
    where
        B: Backend,
    {
        self.area = area;
        let _chunks = Layout::default()
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
            .direction(Direction::Horizontal)
            .split(area);

        let cmp_id = if let Some(id) = self
            .current_room
            .borrow()
            .as_ref()
        {
            Some(id.to_string())
        } else {
            self.messages.first().map(|(id, _msg)| id.to_string())
        };

        let text = self
            .messages
            .iter()
            .filter(|(id, _)| Some(id) == cmp_id.as_ref())
            .map(|(_, msg)| msg.to_string())
            .collect::<Vec<_>>()
            .join("\n");

        Paragraph::new(vec![Text::styled(text, Style::default().fg(Color::Blue))].iter())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Green).modifier(Modifier::BOLD))
                    .title("Messages")
                    .title_style(Style::default().fg(Color::Yellow).modifier(Modifier::BOLD)),
            )
            .wrap(true)
            .render(f, area);
    }
}
