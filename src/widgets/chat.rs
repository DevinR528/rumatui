use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use matrix_sdk::identifiers::RoomId;
use matrix_sdk::Room;
use termion::event::MouseButton;
use tokio::sync::RwLock;
use tui::backend::Backend;
use tui::layout::{Constraint, Direction, Layout, Rect};
use tui::Frame;

use crate::widgets::{message::MessageWidget, rooms::RoomsWidget, RenderWidget};

#[derive(Clone, Debug, Default)]
pub struct ChatWidget {
    pub current_room: Rc<RefCell<Option<RoomId>>>,
    pub room: RoomsWidget,
    pub msgs: MessageWidget,
    pub main_screen: bool,
    pub sending_message: bool,
    pub joining_room: bool,
}

impl ChatWidget {
    pub(crate) async fn set_room_state(&mut self, rooms: HashMap<RoomId, Arc<RwLock<Room>>>) {
        self.msgs.populate_initial_msgs(&rooms).await;
        self.room.populate_rooms(rooms).await;
        self.msgs.current_room = Rc::clone(&self.room.current_room);
        self.current_room = Rc::clone(&self.room.current_room);
    }

    pub fn on_click(&mut self, _btn: MouseButton, _x: u16, _y: u16) {}
}

impl RenderWidget for ChatWidget {
    fn render<B>(&mut self, f: &mut Frame<B>, area: Rect)
    where
        B: Backend,
    {
        let chunks = Layout::default()
            .constraints([Constraint::Percentage(20), Constraint::Percentage(80)].as_ref())
            .direction(Direction::Horizontal)
            .split(area);

        self.room.render(f, chunks[0]);
        self.msgs.render(f, chunks[1]);
    }
}
