use rumatui_tui::{
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph, Text},
    Frame,
};
use termion::event::MouseButton;

use crate::widgets::{login::Loading, RenderWidget};

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RegisterSelect {
    Username = 0,
    Password,
}
impl Default for RegisterSelect {
    fn default() -> Self {
        Self::Username
    }
}
#[derive(Clone, Debug, Default)]
pub struct Register {
    pub selected: RegisterSelect,
    pub username: String,
    pub password: String,
}

#[derive(Clone, Debug, Default)]
pub struct RegisterWidget {
    user_area: Rect,
    password_area: Rect,
    pub register: Register,
    pub registering: bool,
    pub registered: bool,
    pub waiting: Loading,
    pub homeserver: Option<String>,
}

impl RegisterWidget {
    pub(crate) fn try_register(&self) -> bool {
        RegisterSelect::Password == self.register.selected
            && !self.register.password.is_empty()
            && !self.register.username.is_empty()
    }

    pub(crate) fn clear_register(&mut self) {
        // self.register.username.clear();
        // self.register.password.clear();
    }

    /// If right mouse button and clicked within the area of the username or
    /// password field the respective text box is selected.
    pub fn on_click(&mut self, btn: MouseButton, x: u16, y: u16) {
        if let MouseButton::Left = btn {
            if self.user_area.intersects(Rect::new(x, y, 1, 1)) {
                self.register.selected = RegisterSelect::Username;
            } else if self.password_area.intersects(Rect::new(x, y, 1, 1)) {
                self.register.selected = RegisterSelect::Password;
            }
        }
    }
}

impl RenderWidget for RegisterWidget {
    fn render<B>(&mut self, f: &mut Frame<B>, area: Rect)
    where
        B: Backend,
    {
        let chunks = Layout::default()
            .horizontal_margin(40)
            .constraints(
                [
                    Constraint::Percentage(15),
                    Constraint::Percentage(60),
                    Constraint::Percentage(15),
                ]
                .as_ref(),
            )
            .split(area);

        let server = self.homeserver.as_deref().unwrap_or("matrix.org");
        let register = &format!("Register account on {}", server);
        let blk = Block::default()
            .title(register)
            .title_style(Style::default().fg(Color::Green).modifier(Modifier::BOLD))
            .borders(Borders::ALL);
        f.render_widget(blk, chunks[1]);

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

        if self.registering {
            self.waiting.tick(width_chunk1[1].width);
            let blk = Block::default()
                .title("Registering")
                .border_style(Style::default().fg(Color::Magenta).modifier(Modifier::BOLD))
                .borders(Borders::ALL);

            let t = [Text::styled(
                "*".repeat(self.waiting.count),
                Style::default().fg(Color::Magenta),
            )];
            let p = Paragraph::new(t.iter())
                .block(blk)
                .alignment(Alignment::Center);

            f.render_widget(p, width_chunk1[1]);
        } else {
            let (high_user, high_pass) = if self.register.selected == RegisterSelect::Username {
                (
                    Block::default()
                        .title("User Name")
                        .border_style(Style::default().fg(Color::Magenta).modifier(Modifier::BOLD))
                        .borders(Borders::ALL),
                    Block::default().title("Password").borders(Borders::ALL),
                )
            } else {
                (
                    Block::default().title("User Name").borders(Borders::ALL),
                    Block::default()
                        .title("Password")
                        .border_style(Style::default().fg(Color::Magenta).modifier(Modifier::BOLD))
                        .borders(Borders::ALL),
                )
            };

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

            self.user_area = width_chunk1[1];
            self.password_area = width_chunk2[1];

            // User name
            let t = [Text::styled(
                &self.register.username,
                Style::default().fg(Color::Cyan),
            )];
            let p = Paragraph::new(t.iter()).block(high_user);

            f.render_widget(p, width_chunk1[1]);

            // Password from here down
            let t2 = [Text::styled(
                "*".repeat(self.register.password.len()),
                Style::default().fg(Color::Cyan),
            )];
            let p2 = Paragraph::new(t2.iter()).block(high_pass);

            f.render_widget(p2, width_chunk2[1])
        }
    }
}
