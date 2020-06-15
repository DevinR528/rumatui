use std::{
    cell::RefCell,
    collections::HashMap,
    ops::{DerefMut, Index, IndexMut},
    rc::Rc,
    sync::Arc,
};

use itertools::Itertools;
use matrix_sdk::{
    api::r0::directory::PublicRoomsChunk,
    identifiers::{RoomId, UserId},
    Room,
};
use rumatui_tui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, Paragraph, Text},
    Frame,
};
use serde::{Deserialize, Serialize};
use termion::event::MouseButton;
use tokio::sync::RwLock;

use crate::{
    error::Result,
    widgets::{rooms::ListState, RenderWidget},
};

#[derive(Clone, Debug, Default)]
pub struct RoomSearchWidget {
    /// This is the RoomId of the last used room, the room to show on startup.
    pub(crate) current_room: Rc<RefCell<Option<RoomId>>>,
    /// List of displayable room name and room id
    pub names: ListState<PublicRoomsChunk>,
}

impl RoomSearchWidget {
    pub(crate) fn push_room(&mut self, room: PublicRoomsChunk) {
        // TODO only push if it meets criteria?
        self.names.items.push(room);
    }
}

impl RenderWidget for RoomSearchWidget {
    fn render<B>(&mut self, f: &mut Frame<B>, area: Rect)
    where
        B: Backend,
    {
        let chunks = Layout::default()
            .constraints([Constraint::Percentage(100)].as_ref())
            .direction(Direction::Vertical)
            .split(area);

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
            .map(|(i, room)| {
                let name = if let Some(name) = &room.name {
                    name.to_string()
                } else if let Some(canonical) = &room.canonical_alias {
                    canonical.to_string()
                } else {
                    room.aliases
                        .first()
                        .map(|id| id.alias().to_string())
                        .unwrap_or(format!(
                            "room with {} members #{}",
                            room.num_joined_members, i
                        ))
                };
                if i == selected {
                    let style = Style::default()
                        .bg(highlight_style.bg)
                        .fg(highlight_style.fg)
                        .modifier(highlight_style.modifier);
                    Text::styled(format!("{} {}", highlight_symbol, name), style)
                } else {
                    let style = Style::default().fg(Color::Blue);
                    Text::styled(format!(" {}", name), style)
                }
            })
            .skip(offset as usize);
        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Rooms")
                    .border_style(Style::default().fg(Color::Green).modifier(Modifier::BOLD))
                    .title_style(Style::default().fg(Color::Yellow).modifier(Modifier::BOLD)),
            )
            .style(Style::default().fg(Color::Magenta).modifier(Modifier::BOLD));

        f.render_widget(list, chunks[0]);
    }
}
