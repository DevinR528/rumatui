use std::collections::HashMap;
use std::io;
use std::ops::{Index, IndexMut};
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use matrix_sdk::identifiers::{RoomAliasId, RoomId, UserId};
use matrix_sdk::Room;
use tui::backend::Backend;
use tui::layout::{Constraint, Direction, Layout, Rect};
use tui::style::{Color, Modifier, Style};
use tui::widgets::{Block, Borders, List, Paragraph, Tabs, Text, Widget};
use tui::{Frame, Terminal};

use super::RenderWidget;

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

    pub fn select_previous(&mut self) {
        if self.selected != 0 {
            self.selected -= 1;
        }
    }

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
    names: ListState<(String, RoomId)>,
    rooms: HashMap<String, Arc<RwLock<Room>>>,

}

impl RoomsWidget {
    pub(crate) fn populate_rooms(&mut self, rooms: HashMap<String, Arc<RwLock<Room>>>) {
        
    }
}

impl RenderWidget for RoomsWidget {
    fn render<B>(&mut self, f: &mut Frame<B>, area: Rect)
    where
        B: Backend,
    {        
            let list_height = area.height as usize;
    
            // Use highlight_style only if something is selected
            let selected = self.names.selected;
            let highlight_style = Style::default().fg(Color::LightGreen).modifier(Modifier::BOLD);
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
            let item = self.names
                .items
                .iter()
                .enumerate()
                .map(|(i, (id, name))| {   
                    if i == selected {
                        let style = Style::default()
                            .bg(highlight_style.bg)
                            .fg(highlight_style.fg)
                            .modifier(highlight_style.modifier);
                        Text::styled(
                            format!("{} {}", highlight_symbol, name),
                            style,
                        )
                    } else {
                        let style = Style::default().fg(Color::Blue);
                        Text::styled(
                            format!("   {}", name),
                            style,
                        )
                    }
                })
                .skip(offset as usize);
            List::new(item)
                .block(Block::default().borders(Borders::ALL).title("Rooms"))
                .style(Style::default().fg(Color::Magenta).modifier(Modifier::BOLD))
                .render(f, area);
    }
}
