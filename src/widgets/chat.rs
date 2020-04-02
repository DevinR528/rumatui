use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use matrix_sdk::identifiers::RoomId;
use matrix_sdk::Room;
use termion::event::MouseButton;
use tokio::sync::Mutex;
use tui::backend::Backend;
use tui::layout::{Constraint, Direction, Layout, Rect};
use tui::Frame;

use super::app::RenderWidget;
use super::msgs::MessageWidget;
use super::rooms::RoomsWidget;

#[derive(Clone, Debug, Default)]
pub struct ChatWidget {
    pub current_room: Arc<RwLock<Option<crate::RoomIdStr>>>,
    pub room: RoomsWidget,
    pub msgs: MessageWidget,
    pub main_screen: bool,
}

impl ChatWidget {
    pub(crate) async fn set_room_state(
        &mut self,
        rooms: HashMap<String, Arc<Mutex<Room>>>,
        current: Option<RoomId>,
    ) {
        let cur = Arc::new(RwLock::new(current.map(|id| id.to_string())));
        self.current_room = Arc::clone(&cur);
        self.msgs.current_room = Arc::clone(&cur);
        self.room.populate_rooms(rooms, Arc::clone(&cur)).await;
    }

    pub fn on_click(&mut self, btn: MouseButton, x: u16, y: u16) {}
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
