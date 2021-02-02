use std::{cell::RefCell, rc::Rc};

use matrix_sdk::{
    api::r0::directory::get_public_rooms_filtered,
    directory::{PublicRoomsChunk, RoomNetwork},
    identifiers::RoomId,
};
use rumatui_tui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListState as ListTrack, Paragraph, Text},
    Frame,
};

use crate::widgets::{rooms::ListState, RenderWidget};

#[derive(Clone, Debug, Default)]
pub struct RoomSearchWidget {
    /// This is the RoomId of the last used room, the room to show on startup.
    pub(crate) current_room: Rc<RefCell<Option<RoomId>>>,
    /// List of displayable room name and room id
    names: ListState<PublicRoomsChunk>,
    list_state: ListTrack,
    search_term: String,
    next_batch_tkn: Option<String>,
    area: Rect,
}

impl RoomSearchWidget {
    pub(crate) fn try_room_search(&self) -> bool {
        !self.search_term.is_empty()
    }

    pub(crate) fn search_term(&self) -> &str {
        &self.search_term
    }

    pub(crate) fn next_batch_tkn(&self) -> Option<&str> {
        self.next_batch_tkn.as_deref()
    }

    pub(crate) fn set_current_room_id(
        &mut self,
        room: Rc<RefCell<Option<RoomId>>>,
    ) -> Rc<RefCell<Option<RoomId>>> {
        let copy = Rc::clone(&room);
        self.current_room = room;
        copy
    }

    pub(crate) fn push_search_text(&mut self, ch: char) {
        // TODO only push if it meets criteria?
        self.search_term.push(ch);
    }

    pub(crate) fn pop_search_text(&mut self) {
        self.search_term.pop();
    }

    pub(crate) fn clear_search_result(&mut self) {
        self.names.clear();
    }

    pub(crate) fn selected_room(&self) -> Option<RoomId> {
        self.names.get_selected().map(|r| r.room_id.clone())
    }

    pub(crate) fn room_search_results(&mut self, response: get_public_rooms_filtered::Response) {
        self.next_batch_tkn = response.next_batch.clone();
        // TODO only push if it meets criteria?
        for room in response.chunk {
            self.names.items.push(room);
        }
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
    }

    /// Moves the selection up the list
    pub fn select_previous(&mut self) {
        self.names.select_previous();
        self.list_state.select(Some(self.names.selected_idx()))
    }

    /// Passes the remembered filter, room network, and since token to make
    /// the room search request again.
    pub fn next_request(&mut self) -> Option<(String, RoomNetwork<'_>, String)> {
        if let Some(tkn) = self.next_batch_tkn() {
            Some((
                self.search_term.to_string(),
                RoomNetwork::Matrix,
                tkn.to_string(),
            ))
        } else {
            None
        }
    }
}

impl RenderWidget for RoomSearchWidget {
    fn render<B>(&mut self, f: &mut Frame<B>, area: Rect)
    where
        B: Backend,
    {
        let chunks = Layout::default()
            .constraints(
                [
                    Constraint::Percentage(20),
                    Constraint::Percentage(70),
                    Constraint::Percentage(10),
                ]
                .as_ref(),
            )
            .direction(Direction::Vertical)
            .split(area);

        // set the area of the scroll-able window (the rooms list)
        self.area = chunks[1];

        let mut details = String::new();
        let mut found_topic = None::<String>;

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
                    found_topic = room.topic.clone();
                    details = format!(
                        "Can guests participate: {}    Members: {}",
                        if room.guest_can_join { "yes" } else { "no" },
                        room.num_joined_members
                    );
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
                    .title("Public Rooms")
                    .border_style(Style::default().fg(Color::Green).modifier(Modifier::BOLD))
                    .title_style(Style::default().fg(Color::Yellow).modifier(Modifier::BOLD)),
            )
            .style(Style::default().fg(Color::Magenta).modifier(Modifier::BOLD));
        f.render_widget(list, chunks[1]);

        let mut topic = found_topic.unwrap_or_default();
        topic.push_str("    ");

        let t = vec![
            Text::styled(&topic, Style::default().fg(Color::Blue)),
            Text::styled(&details, Style::default().fg(Color::LightGreen)),
        ];
        let room_topic = Paragraph::new(t.iter())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Green).modifier(Modifier::BOLD))
                    .title("Room Topic")
                    .title_style(Style::default().fg(Color::Yellow).modifier(Modifier::BOLD)),
            )
            .wrap(true);
        f.render_widget(room_topic, chunks[0]);

        let t3 = vec![
            Text::styled(&self.search_term, Style::default().fg(Color::Blue)),
            Text::styled(
                "<",
                Style::default()
                    .fg(Color::LightGreen)
                    .modifier(Modifier::RAPID_BLINK),
            ),
        ];
        let text_box = Paragraph::new(t3.iter())
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Green).modifier(Modifier::BOLD))
                    .title("Send")
                    .title_style(Style::default().fg(Color::Yellow).modifier(Modifier::BOLD)),
            )
            .wrap(true);

        f.render_widget(text_box, chunks[2]);
    }
}
