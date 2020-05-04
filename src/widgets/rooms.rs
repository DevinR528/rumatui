use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::{Index, IndexMut};
use std::rc::Rc;
use std::sync::Arc;

use itertools::Itertools;
use matrix_sdk::identifiers::{RoomId, UserId};
use matrix_sdk::Room;
use serde::{Deserialize, Serialize};
use termion::event::MouseButton;
use tokio::sync::RwLock;
use tui::backend::Backend;
use tui::layout::{Constraint, Direction, Layout, Rect};
use tui::style::{Color, Modifier, Style};
use tui::widgets::{Block, Borders, List, Paragraph, Text};
use tui::Frame;

use crate::widgets::RenderWidget;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ListState<I> {
    pub items: Vec<I>,
    pub selected: usize,
}

impl<I> Default for ListState<I> {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl<I> ListState<I> {
    pub fn new(items: Vec<I>) -> ListState<I> {
        ListState { items, selected: 0 }
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Scrolls back up the list
    pub fn select_previous(&mut self) {
        if self.selected != 0 {
            self.selected -= 1;
        }
    }

    /// Scrolls down the list
    pub fn select_next(&mut self) {
        if self.is_empty() {
            return;
        }
        if self.selected < self.len() - 1 {
            self.selected += 1
        }
    }
    pub fn get_selected(&self) -> Option<&I> {
        self.items.get(self.selected)
    }
    pub fn get_selected_mut(&mut self) -> Option<&mut I> {
        self.items.get_mut(self.selected)
    }

    pub fn iter(&self) -> impl Iterator<Item = &I> {
        self.items.iter()
    }
}

impl<I> Index<usize> for ListState<I> {
    type Output = I;
    fn index(&self, idx: usize) -> &Self::Output {
        &self.items[idx]
    }
}
impl<I> IndexMut<usize> for ListState<I> {
    fn index_mut(&mut self, idx: usize) -> &mut Self::Output {
        &mut self.items[idx]
    }
}

#[derive(Clone, Debug)]
pub struct Invitation {
    pub(crate) room_id: RoomId,
    room_name: String,
    sender: UserId,
}

pub enum Invite {
    Accept,
    Decline,
    NoClick,
}
// TODO split RoomsWidget into render and state halves. RoomRender has the methods to filter
// and populate ListState using the rooms HashMap. RoomState or RoomData?? will populate and keep track of
// state

#[derive(Clone, Debug, Default)]
pub struct RoomsWidget {
    area: Rect,
    yes_area: Rect,
    no_area: Rect,
    /// This is the RoomId of the last used room, the room to show on startup.
    pub(crate) current_room: Rc<RefCell<Option<RoomId>>>,
    /// List of displayable room name and room id
    pub names: ListState<(String, RoomId)>,
    /// Map of room id and matrix_sdk::Room
    pub(crate) rooms: HashMap<RoomId, Arc<RwLock<Room>>>,
    /// When a user receives an invitation an alert pops up in the `RoomsWidget` pane
    // this signals to show that pop up.
    pub(crate) invite: Option<Invitation>,
}

impl RoomsWidget {
    /// Updates the `RoomWidget` state to reflect the current client state.
    ///
    /// ## Arguments
    ///  * rooms - A `HashMap` of room_id to `Room`.
    pub(crate) async fn populate_rooms(&mut self, rooms: HashMap<RoomId, Arc<RwLock<Room>>>) {
        self.rooms = rooms.clone();
        let mut items: Vec<(String, RoomId)> = Vec::default();
        for (id, room) in &rooms {
            // filter duplicate rooms
            if items.iter().any(|(_name, rid)| id == rid) {
                continue;
            }
            let r = room.read().await;
            // filter tombstoned rooms
            if r.tombstone.is_some() {
                continue;
            }
            items.push((r.calculate_name(), id.clone()));
        }

        if let Some((_name, id)) = items.first() {
            *self.current_room.borrow_mut() = Some(id.clone());
        }

        self.names = ListState::new(items);
    }

    pub(crate) async fn add_room(&mut self, room: Arc<RwLock<Room>>) {
        let r = room.read().await;
        let name = r.calculate_name();
        let room_id = r.room_id.clone();

        self.rooms.insert(room_id.clone(), Arc::clone(&room));
        self.names.items.push((name, room_id));
    }

    pub(crate) fn remove_room(&mut self, room_id: RoomId) {
        // self.rooms.remove(&room_id);
        if let Some(idx) = self.names.items.iter().position(|(_, id)| &room_id == id) {
            self.names.items.remove(idx);
        }
    }

    pub(crate) fn update_room(&mut self, name: String, room_id: RoomId) {
        if let Some(idx) = self.names.items.iter().position(|(_, id)| &room_id == id) {
            self.names.items[idx] = (name, room_id);
        }
    }

    pub(crate) async fn invited(&mut self, sender: UserId, room: Arc<RwLock<Room>>) {
        let r = room.read().await;
        let room_id = r.room_id.clone();
        let room_name = r.calculate_name();
        self.invite = Some(Invitation {
            sender,
            room_id,
            room_name,
        });
    }

    pub(crate) fn remove_invite(&mut self) {
        self.invite.take();
    }

    pub fn on_click(&mut self, _btn: MouseButton, x: u16, y: u16) -> Invite {
        if self.yes_area.intersects(Rect::new(x, y, 1, 1)) {
            return Invite::Accept;
        }
        if self.no_area.intersects(Rect::new(x, y, 1, 1)) {
            return Invite::Decline;
        }
        Invite::NoClick
    }

    pub fn on_scroll_up(&mut self, x: u16, y: u16) -> bool {
        if self.area.intersects(Rect::new(x, y, 1, 1)) {
            self.select_previous();
            return true;
        }
        false
    }

    pub fn on_scroll_down(&mut self, x: u16, y: u16) -> bool {
        if self.area.intersects(Rect::new(x, y, 1, 1)) {
            self.select_next();
            return true;
        }
        false
    }

    /// Moves selection down the list
    pub fn select_next(&mut self) {
        self.names.select_next();
        if let Some((_name, id)) = self.names.get_selected() {
            *self.current_room.borrow_mut() = Some(id.clone());
        }
    }

    /// Moves the selection up the list
    pub fn select_previous(&mut self) {
        self.names.select_previous();
        if let Some((_name, id)) = self.names.get_selected() {
            *self.current_room.borrow_mut() = Some(id.clone());
        }
    }
}

impl RenderWidget for RoomsWidget {
    fn render<B>(&mut self, f: &mut Frame<B>, area: Rect)
    where
        B: Backend,
    {
        let chunks = if self.invite.is_some() {
            Layout::default()
                .constraints([Constraint::Percentage(60), Constraint::Percentage(40)].as_ref())
                .split(area)
        } else {
            Layout::default()
                .constraints([Constraint::Percentage(100)].as_ref())
                .split(area)
        };

        self.area = chunks[0];
        let list_height = self.area.height as usize;

        // Use highlight_style only if something is selected
        let selected = self.names.selected;
        let highlight_style = Style::default()
            .fg(Color::LightGreen)
            .modifier(Modifier::BOLD);
        let highlight_symbol = ">>";
        // Make sure the list show the selected item
        let offset = {
            if selected >= list_height {
                selected - list_height + 1
            } else {
                0
            }
        };

        // Render items
        let items = self
            .names
            .items
            .iter()
            .unique_by(|(_, id)| id)
            .enumerate()
            .map(|(i, (name, _id))| {
                if i == selected {
                    let style = Style::default()
                        .bg(highlight_style.bg)
                        .fg(highlight_style.fg)
                        .modifier(highlight_style.modifier);
                    Text::styled(format!("{} {}", highlight_symbol, name), style)
                } else {
                    let style = Style::default().fg(Color::Blue);
                    Text::styled(format!("   {}", name), style)
                }
            })
            .skip(offset as usize);
        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Rooms")
                    .border_style(Style::default().fg(Color::Green).modifier(Modifier::BOLD))
                    .title("Messages")
                    .title_style(Style::default().fg(Color::Yellow).modifier(Modifier::BOLD)),
            )
            .style(Style::default().fg(Color::Magenta).modifier(Modifier::BOLD));

        f.render_widget(list, chunks[0]);

        if let Some(invite) = self.invite.as_ref() {
            let label_text = format!("Invited to {}", invite.room_name);
            let label = Block::default().title(&label_text);
            f.render_widget(label, chunks[1]);

            let height_chunk = Layout::default()
                .direction(Direction::Vertical)
                .constraints(
                    [
                        Constraint::Percentage(20),
                        Constraint::Percentage(30),
                        Constraint::Percentage(30),
                        Constraint::Percentage(20),
                    ]
                    .as_ref(),
                )
                .split(chunks[1]);

            let width_chunk1 = Layout::default()
                .direction(Direction::Horizontal)
                .constraints(
                    [
                        Constraint::Percentage(25),
                        Constraint::Percentage(50),
                        Constraint::Percentage(25),
                    ]
                    .as_ref(),
                )
                .split(height_chunk[1]);

            let yes = Block::default().title("Accept").borders(Borders::ALL);
            let no = Block::default().title("Decline").borders(Borders::ALL);

            // password width using password height
            let width_chunk2 = Layout::default()
                .direction(Direction::Horizontal)
                .constraints(
                    [
                        Constraint::Percentage(25),
                        Constraint::Percentage(50),
                        Constraint::Percentage(25),
                    ]
                    .as_ref(),
                )
                .split(height_chunk[2]);

            self.yes_area = width_chunk1[1];
            self.no_area = width_chunk2[1];

            let t = [Text::styled(
                "Accept invite",
                Style::default().fg(Color::Cyan),
            )];
            let ok = Paragraph::new(t.iter()).block(yes);
            f.render_widget(ok, width_chunk1[1]);

            // Password from here down
            let t2 = [Text::styled(
                "Decline invite",
                Style::default().fg(Color::Cyan),
            )];
            let nope = Paragraph::new(t2.iter()).block(no);
            f.render_widget(nope, width_chunk2[1])
        }
    }
}
