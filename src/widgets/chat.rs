use std::cell::{Ref, RefCell};
use std::collections::HashMap;
use std::ops::Deref;
use std::rc::Rc;
use std::sync::Arc;
use std::time::SystemTime;

use matrix_sdk::events::room::message::MessageEventContent;
use matrix_sdk::identifiers::{EventId, RoomId, UserId};
use matrix_sdk::Room;
use rumatui_tui::backend::Backend;
use rumatui_tui::layout::{Constraint, Direction, Layout, Rect};
use rumatui_tui::Frame;
use termion::event::MouseButton;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::widgets::{
    message::{Message, MessageWidget},
    rooms::{Invitation, Invite, RoomsWidget},
    RenderWidget,
};

#[derive(Clone, Debug, Default)]
pub struct ChatWidget {
    current_room: Rc<RefCell<Option<RoomId>>>,
    me: Option<UserId>,
    room: RoomsWidget,
    msgs: MessageWidget,
    main_screen: bool,
    sending_message: bool,
    joining_room: bool,
    leaving_room: bool,
}

impl ChatWidget {
    pub(crate) fn is_main_screen(&self) -> bool {
        self.main_screen
    }

    pub(crate) fn is_joining_room(&self) -> bool {
        self.joining_room
    }

    pub(crate) fn is_leaving_room(&self) -> bool {
        self.leaving_room
    }

    pub(crate) fn is_sending_message(&self) -> bool {
        self.sending_message
    }

    pub(crate) fn set_main_screen(&mut self, value: bool) {
        self.main_screen = value;
    }

    pub(crate) fn set_joining_room(&mut self, value: bool) {
        self.joining_room = value;
    }

    pub(crate) fn set_leaving_room(&mut self, value: bool) {
        self.leaving_room = value;
    }

    pub(crate) fn set_sending_message(&mut self, value: bool) {
        self.sending_message = value;
    }

    pub(crate) fn is_current_room(&self, room: &RoomId) -> bool {
        self.as_current_room_id().as_ref() == Some(room)
    }

    pub(crate) fn as_current_room_id(&self) -> Ref<'_, Option<RoomId>> {
        self.current_room.borrow()
    }

    pub(crate) fn to_current_room_id(&self) -> Option<RoomId> {
        self.current_room.borrow().clone()
    }

    pub(crate) fn set_current_room_id(&mut self, room: &RoomId) {
        self.current_room = Rc::new(RefCell::new(Some(room.clone())));
        self.msgs.current_room = Rc::clone(&self.current_room);
    }

    pub(crate) fn as_current_user(&self) -> Option<&UserId> {
        self.me.as_ref()
    }

    pub(crate) fn into_current_user(&self) -> Option<UserId> {
        self.me.clone()
    }

    pub(crate) fn set_current_user(&mut self, user: &UserId) {
        self.me = Some(user.clone());
        self.msgs.me = Some(user.clone());
    }

    pub(crate) fn as_invite(&self) -> Option<&Invitation> {
        self.room.invite.as_ref()
    }

    pub(crate) fn rooms(&self) -> &HashMap<RoomId, Arc<RwLock<Room>>> {
        &self.room.rooms
    }

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

    pub(crate) fn update_room(&mut self, name: &str, room: &RoomId) {
        self.room.update_room(name, room)
    }

    pub(crate) fn room_on_click(&mut self, btn: MouseButton, x: u16, y: u16) -> Invite {
        self.room.on_click(btn, x, y)
    }

    pub(crate) fn room_on_scroll_up(&mut self, x: u16, y: u16) -> bool {
        self.room.on_scroll_up(x, y)
    }

    pub(crate) fn room_on_scroll_down(&mut self, x: u16, y: u16) -> bool {
        self.room.on_scroll_down(x, y)
    }

    pub(crate) fn room_select_previous(&mut self) {
        self.room.select_previous()
    }

    pub(crate) fn room_select_next(&mut self) {
        self.room.select_next()
    }

    pub(crate) fn remove_invite(&mut self) {
        self.room.remove_invite()
    }

    pub(crate) async fn add_room(&mut self, room: Arc<RwLock<Room>>) {
        self.room.add_room(room).await
    }

    pub(crate) fn remove_room(&mut self, room: &RoomId) {
        self.room.remove_room(room)
    }

    pub(crate) async fn invited(&mut self, sender: UserId, room: Arc<RwLock<Room>>) {
        self.room.invited(sender, room).await
    }

    pub(crate) fn msgs_on_click(&mut self, btn: MouseButton, x: u16, y: u16) -> bool {
        self.msgs.on_click(btn, x, y)
    }

    pub(crate) fn msgs_on_scroll_up(&mut self, x: u16, y: u16) -> bool {
        self.msgs.on_scroll_up(x, y)
    }

    pub(crate) fn msgs_on_scroll_down(&mut self, x: u16, y: u16) {
        self.msgs.on_scroll_down(x, y)
    }

    pub(crate) fn reset_scroll(&mut self) {
        self.msgs.reset_scroll()
    }

    pub(crate) fn add_char(&mut self, ch: char) {
        self.msgs.add_char(ch)
    }

    pub(crate) fn remove_char(&mut self) {
        self.msgs.remove_char()
    }

    pub(crate) fn add_notify(&mut self, msg: &str) {
        self.msgs.add_notify(msg)
    }

    pub(crate) fn add_message(&mut self, msg: Message, room: &RoomId) {
        self.msgs.add_message(msg, room)
    }

    pub(crate) fn echo_sent_msg(
        &mut self,
        id: &RoomId,
        name: String,
        homeserver: &str,
        uuid: Uuid,
        content: MessageEventContent,
    ) {
        self.msgs.echo_sent_msg(id, name, homeserver, uuid, content)
    }

    pub(crate) fn edit_message(&mut self, room: &RoomId, event: &EventId, new_msg: String) {
        self.msgs.edit_message(room, event, new_msg)
    }

    pub(crate) fn clear_send_msg(&mut self) {
        self.msgs.clear_send_msg()
    }

    pub(crate) fn get_sending_message(&self) -> anyhow::Result<MessageEventContent> {
        self.msgs.get_sending_message()
    }

    pub(crate) async fn check_unread(&mut self, room: Arc<RwLock<Room>>) -> Option<EventId> {
        self.msgs.check_unread(room.read().await.deref())
    }

    pub(crate) fn read_receipt(
        &mut self,
        last_interaction: SystemTime,
    ) -> Option<(EventId, RoomId)> {
        self.msgs.read_receipt(last_interaction)
    }

    pub(crate) fn read_to_end(&mut self, event: &EventId) -> bool {
        self.msgs.read_to_end(event)
    }

    pub(crate) fn last_3_msg_event_ids(&self) -> impl Iterator<Item = &EventId> {
        self.msgs.last_3_msg_event_ids()
    }
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
