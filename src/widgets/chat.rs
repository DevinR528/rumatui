
use std::collections::HashMap;
use std::sync::Arc;

use matrix_sdk::Room;
use tui::backend::Backend;
use tui::layout::{Constraint, Direction, Layout, Rect};
use tui::{Frame};
use tokio::sync::Mutex;

use super::msgs::MessageWidget;
use super::rooms::RoomsWidget;
use super::RenderWidget;

#[derive(Clone, Debug, Default)]
pub struct ChatWidget {
    pub room: RoomsWidget,
    pub msgs: MessageWidget,
}

impl ChatWidget {
    pub(crate) fn set_room_state(&mut self, rooms: HashMap<String, Arc<Mutex<Room>>>) {
        self.room.populate_rooms(rooms);
    }
}

impl matrix_sdk::EventEmitter for ChatWidget {
    
}

impl RenderWidget for ChatWidget {
    fn render<B>(&mut self, f: &mut Frame<B>, area: Rect)
    where
        B: Backend,
    {
        let chunks = Layout::default()
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)].as_ref())
            .direction(Direction::Horizontal)
            .split(area);

        self.room.render(f, chunks[0]);
        self.msgs.render(f, chunks[1]);
    }
}
