use std::{
    cell::{Ref, RefCell},
    collections::HashMap,
    ops::Deref,
    rc::Rc,
    sync::Arc,
    time::SystemTime,
};

use matrix_sdk::{
    api::r0::directory::get_public_rooms_filtered::{self, RoomNetwork},
    events::room::message::MessageEventContent,
    identifiers::{EventId, RoomId, UserId},
    Room,
};
use rumatui_tui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    Frame,
};
use termion::event::MouseButton;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::{
    error::Result,
    widgets::{
        message::{Message, MessageWidget},
        room_search::RoomSearchWidget,
        rooms::{Invitation, Invite, RoomsWidget},
        RenderWidget,
    },
};

#[derive(Clone, Debug, Default)]
pub struct ChatWidget {
    current_room: Rc<RefCell<Option<RoomId>>>,
    me: Option<UserId>,
    pub rooms_widget: RoomsWidget,
    messages_widget: MessageWidget,
    room_search_widget: RoomSearchWidget,
    room_search: bool,
    main_screen: bool,
    sending_message: bool,
    joining_room: bool,
    leaving_room: bool,
}

impl ChatWidget {
    pub(crate) fn is_room_search(&self) -> bool {
        self.room_search
    }

    pub(crate) fn set_room_search(&mut self, value: bool) {
        self.room_search = value;
    }

    pub(crate) fn push_search_text(&mut self, ch: char) {
        self.room_search_widget.push_search_text(ch);
    }

    pub(crate) fn pop_search_text(&mut self) {
        self.room_search_widget.pop_search_text();
    }

    pub(crate) fn try_room_search(&self) -> bool {
        self.room_search_widget.try_room_search()
    }

    pub(crate) fn search_term(&self) -> &str {
        self.room_search_widget.search_term()
    }

    pub(crate) fn selected_room_search(&mut self) -> Option<RoomId> {
        self.room_search_widget.selected_room()
    }

    pub(crate) fn clear_room_search(&mut self) {
        self.room_search_widget.clear_search_result()
    }

    pub(crate) fn room_search_results(&mut self, resp: get_public_rooms_filtered::Response) {
        self.room_search_widget.room_search_results(resp)
    }

    pub(crate) fn room_search_scroll_up(&mut self, x: u16, y: u16) -> bool {
        self.room_search_widget.on_scroll_up(x, y)
    }

    pub(crate) fn room_search_scroll_down(&mut self, x: u16, y: u16) -> bool {
        self.room_search_widget.on_scroll_down(x, y)
    }

    pub(crate) fn room_search_select_previous(&mut self) {
        self.room_search_widget.select_previous()
    }

    pub(crate) fn room_search_select_next(&mut self) {
        self.room_search_widget.select_next()
    }

    pub(crate) fn room_search_next_request(&mut self) -> Option<(String, RoomNetwork, String)> {
        self.room_search_widget.next_request()
    }
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

    pub(crate) async fn set_room_state(
        &mut self,
        rooms: Arc<RwLock<HashMap<RoomId, Arc<RwLock<Room>>>>>,
    ) {
        tracing::info!("setting room state");
        self.messages_widget
            .populate_initial_msgs(rooms.read().await.deref())
            .await;
        self.rooms_widget.populate_rooms(rooms).await;
        self.messages_widget.current_room = Rc::clone(&self.rooms_widget.current_room);
        self.current_room = Rc::clone(&self.rooms_widget.current_room);
        self.room_search_widget.current_room = Rc::clone(&self.rooms_widget.current_room);
    }

    pub(crate) fn set_current_room_id(&mut self, room: &RoomId) {
        tracing::trace!("setting room id {}", &room);
        self.rooms_widget.set_room_selected(room);
        *self.current_room.borrow_mut() = Some(room.clone());
    }


    pub(crate) fn quit_quick_select_room(&mut self) {
        self.rooms_widget.quit_quick_select_room();
    }

    pub(crate) fn start_quick_select_room(&mut self) {
        self.rooms_widget.start_quick_select_room();
    }

    pub(crate) fn is_quick_select(&self) -> bool {
        self.rooms_widget.is_quick_select()
    }

    pub(crate) fn as_current_user(&self) -> Option<&UserId> {
        self.me.as_ref()
    }

    pub(crate) fn to_current_user(&self) -> Option<UserId> {
        self.me.clone()
    }

    pub(crate) fn set_current_user(&mut self, user: &UserId) {
        self.me = Some(user.clone());
        self.messages_widget.me = Some(user.clone());
    }

    pub(crate) fn as_invite(&self) -> Option<&Invitation> {
        self.rooms_widget.invite.as_ref()
    }

    pub(crate) fn rooms(&self) -> &HashMap<RoomId, Arc<RwLock<Room>>> {
        &self.rooms_widget.rooms
    }

    pub(crate) fn update_room(&mut self, name: &str, room: &RoomId) {
        tracing::info!("update room state {}", &room);
        self.rooms_widget.update_room(name, room)
    }

    pub(crate) fn room_on_click(&mut self, btn: MouseButton, x: u16, y: u16) -> Invite {
        self.rooms_widget.on_click(btn, x, y)
    }

    pub(crate) fn room_on_scroll_up(&mut self, x: u16, y: u16) -> bool {
        self.rooms_widget.on_scroll_up(x, y)
    }

    pub(crate) fn room_on_scroll_down(&mut self, x: u16, y: u16) -> bool {
        self.rooms_widget.on_scroll_down(x, y)
    }

    pub(crate) fn room_select_previous(&mut self) {
        self.rooms_widget.select_previous()
    }

    pub(crate) fn room_select_next(&mut self) {
        self.rooms_widget.select_next()
    }

    pub(crate) fn remove_invite(&mut self) {
        self.rooms_widget.remove_invite()
    }

    pub(crate) async fn add_room(&mut self, room: Arc<RwLock<Room>>) {
        tracing::info!("adding room to room list");
        self.messages_widget.add_room(Arc::clone(&room)).await;
        self.rooms_widget.add_room(room).await
    }

    pub(crate) fn remove_room(&mut self, room: &RoomId) {
        tracing::info!("removing room to room list");
        self.rooms_widget.remove_room(room)
    }

    pub(crate) async fn invited(&mut self, sender: UserId, room: Arc<RwLock<Room>>) {
        tracing::info!("{} was invited to a room", sender);
        self.rooms_widget.invited(sender, room).await
    }

    pub(crate) fn msgs_on_click(&mut self, btn: MouseButton, x: u16, y: u16) -> bool {
        self.messages_widget.on_click(btn, x, y)
    }

    pub(crate) fn msgs_on_scroll_up(&mut self, x: u16, y: u16) -> bool {
        self.messages_widget.on_scroll_up(x, y)
    }

    pub(crate) fn msgs_on_scroll_down(&mut self, x: u16, y: u16) {
        self.messages_widget.on_scroll_down(x, y)
    }

    pub(crate) fn reset_scroll(&mut self) {
        self.messages_widget.reset_scroll()
    }

    pub(crate) fn add_char(&mut self, ch: char) {
        if self.is_quick_select() {
            self.rooms_widget.quick_select_add_char(ch)
        } else {
            self.messages_widget.add_char(ch)
        }
    }

    pub(crate) fn remove_char(&mut self) {
        if self.is_quick_select() {
            self.rooms_widget.quick_select_remove_char()
        } else {
            self.messages_widget.remove_char()
        }
    }

    pub(crate) fn add_notify(&mut self, msg: &str) {
        self.messages_widget.add_notify(msg)
    }

    pub(crate) fn set_reaction_event(
        &mut self,
        room: &RoomId,
        relates_to: &EventId,
        event_id: &EventId,
        reaction: &str,
    ) {
        tracing::info!("setting reaction");
        self.messages_widget
            .set_reaction_event(room, relates_to, event_id, reaction)
    }

    pub(crate) fn add_message(&mut self, msg: Message, room: &RoomId) {
        tracing::info!("adding message in room {}", &room);
        self.messages_widget.add_message(msg, room)
    }

    pub(crate) fn echo_sent_msg(
        &mut self,
        id: &RoomId,
        name: String,
        uuid: Uuid,
        content: MessageEventContent,
    ) {
        tracing::info!("echoing sent message");
        self.messages_widget.echo_sent_msg(id, name, uuid, content)
    }

    pub(crate) fn edit_message(&mut self, room: &RoomId, event: &EventId, new_msg: String) {
        tracing::info!("message edit in {}", &room);
        self.messages_widget.edit_message(room, event, new_msg)
    }

    pub(crate) fn redaction_event(&mut self, room: &RoomId, event: &EventId) {
        tracing::info!("redaction event in {}", &room);
        self.messages_widget.redaction_event(room, event)
    }

    pub(crate) fn clear_send_msg(&mut self) {
        self.messages_widget.clear_send_msg()
    }

    pub(crate) fn get_sending_message(&self) -> Result<MessageEventContent> {
        self.messages_widget.get_sending_message()
    }

    /// `check_unread` is used when the user is active in a room, we check for any messages
    /// that have not been seen and mark them as seen by sending a read marker/read receipt.
    pub(crate) async fn check_unread(&mut self, room: Arc<RwLock<Room>>) -> Option<EventId> {
        self.messages_widget.check_unread(room.read().await.deref())
    }

    /// `read_receipt` is used when a message comes in and the user is
    /// active we immediately send a read marker.
    pub(crate) fn read_receipt(
        &mut self,
        last_interaction: SystemTime,
        room: &RoomId,
    ) -> Option<EventId> {
        self.messages_widget.read_receipt(last_interaction, room)
    }

    pub(crate) fn read_to_end(&mut self, room: &RoomId, event: &EventId) -> bool {
        self.messages_widget.read_to_end(room, event)
    }

    pub(crate) fn last_3_msg_event_ids(&self, room: &RoomId) -> Vec<&EventId> {
        self.messages_widget.last_3_msg_event_ids(room)
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

        self.rooms_widget.render(f, chunks[0]);

        if self.is_room_search() {
            self.room_search_widget.render(f, chunks[1]);
        } else {
            self.messages_widget.render(f, chunks[1]);
        }
    }
}
