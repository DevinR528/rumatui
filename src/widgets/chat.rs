use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Deref;
use std::rc::Rc;
use std::sync::Arc;

use matrix_sdk::identifiers::{RoomId, UserId};
use matrix_sdk::Room;
use rumatui_tui::backend::Backend;
use rumatui_tui::layout::{Constraint, Direction, Layout, Rect};
use rumatui_tui::Frame;
use termion::event::MouseButton;
use tokio::sync::RwLock;

use crate::widgets::{message::MessageWidget, rooms::RoomsWidget, RenderWidget};

// TODO make ChatWidget have all methods to delegate to widgets.
// Once widgets are broken up into state/render ChatWidget will be
// responsible for calling the needed method.
// TODO remove field access of all widgets, use methods.

#[derive(Clone, Debug, Default)]
pub struct ChatWidget {
    pub current_room: Rc<RefCell<Option<RoomId>>>,
    pub(crate) me: Option<UserId>,
    pub room: RoomsWidget,
    pub msgs: MessageWidget,
    pub main_screen: bool,
    pub sending_message: bool,
    pub joining_room: bool,
    pub leaving_room: bool,
}

impl ChatWidget {
    pub(crate) async fn set_room_state(
        &mut self,
        rooms: Arc<RwLock<HashMap<RoomId, Arc<RwLock<Room>>>>>,
    ) {
        self.msgs
            .populate_initial_msgs(rooms.read().await.deref())
            .await;
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
