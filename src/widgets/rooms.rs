use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::{Index, IndexMut};
use std::rc::Rc;
use std::sync::Arc;

use matrix_sdk::identifiers::RoomId;
use matrix_sdk::Room;
use serde::{Deserialize, Serialize};
use termion::event::MouseButton;
use tokio::sync::Mutex;
use tui::backend::Backend;
use tui::layout::Rect;
use tui::style::{Color, Modifier, Style};
use tui::widgets::{Block, Borders, List, Text};
use tui::Frame;

use super::app::RenderWidget;

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

#[derive(Clone, Debug, Default)]
pub struct RoomsWidget {
    area: Rect,
    /// This is the RoomId of the last used room, the room to show on startup.
    pub(crate) current_room: Rc<RefCell<Option<RoomId>>>,
    /// List of displayable room name and room id
    pub names: ListState<(String, RoomId)>,
    /// Map of room id and matrix_sdk::Room
    rooms: HashMap<RoomId, Arc<Mutex<Room>>>,
}

impl RoomsWidget {
    /// Updates the `RoomWidget` state to reflect the current client state.
    ///
    /// ## Arguments
    ///  * rooms - A `HashMap` of room_id to `Room`.
    ///  * current is the current room id controlled by the ChatWidget.
    pub(crate) async fn populate_rooms(&mut self, rooms: HashMap<RoomId, Arc<Mutex<Room>>>) {
        self.rooms = rooms.clone();
        let mut items: Vec<(String, RoomId)> = Vec::default();
        for (id, room) in &rooms {
            let r = room.lock().await;
            // TODO when RoomId impls AsRef<str> cleanup
            if items.iter().any(|(_name, rid)| id == rid) {
                continue;
            }

            items.push((r.calculate_name(), id.clone()));
        }

        if let Some((_name, id)) = items.first() {
            *self.current_room.borrow_mut() = Some(id.clone());
        }

        self.names = ListState::new(items);
    }

    pub fn on_click(&mut self, _btn: MouseButton, x: u16, y: u16) {
        if self.area.intersects(Rect::new(x, y, 1, 1)) {}
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
        self.area = area;
        let list_height = area.height as usize;

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
            .block(Block::default().borders(Borders::ALL).title("Rooms"))
            .style(Style::default().fg(Color::Magenta).modifier(Modifier::BOLD));

        f.render_widget(list, area);
    }
}
