use rumatui_tui::{
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph, Text},
    Frame,
};
use termion::event::MouseButton;

use crate::widgets::RenderWidget;

#[derive(Clone, Copy, Debug)]
pub struct Loading {
    pub count: usize,
    pub add: bool,
}

impl Default for Loading {
    fn default() -> Self {
        Self {
            count: 1,
            add: true,
        }
    }
}

impl Loading {
    pub fn tick(&mut self, max: u16) {
        let max = (max - 1) as usize;
        if self.count > max {
            self.add = false;
        }
        if self.count == 1 {
            self.add = true;
        }

        if self.add {
            self.count += 1;
        } else {
            self.count -= 1;
        }
    }
}
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LoginSelect {
    Username = 0,
    Password,
}
impl Default for LoginSelect {
    fn default() -> Self {
        Self::Username
    }
}
#[derive(Clone, Debug, Default)]
pub struct Login {
    pub selected: LoginSelect,
    pub username: String,
    pub password: String,
}

#[derive(Clone, Debug, Default)]
pub struct LoginWidget {
    user_area: Rect,
    password_area: Rect,
    pub login: Login,
    pub logging_in: bool,
    pub logged_in: bool,
    pub waiting: Loading,
    pub homeserver: Option<String>,
}

impl LoginWidget {
    pub(crate) fn try_login(&self) -> bool {
        LoginSelect::Password == self.login.selected
            && !self.login.password.is_empty()
            && !self.login.username.is_empty()
    }

    pub(crate) fn clear_login(&mut self) {
        // self.login.username.clear();
        // self.login.password.clear();
    }

    /// If right mouse button and clicked within the area of the username or
    /// password field the respective text box is selected.
    pub fn on_click(&mut self, btn: MouseButton, x: u16, y: u16) {
        if let MouseButton::Left = btn {
            if self.user_area.intersects(Rect::new(x, y, 1, 1)) {
                self.login.selected = LoginSelect::Username;
            } else if self.password_area.intersects(Rect::new(x, y, 1, 1)) {
                self.login.selected = LoginSelect::Password;
            }
        }
    }
}

impl RenderWidget for LoginWidget {
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
        let login = &format!("Log in to {}", server);
        let blk = Block::default()
            .title(login)
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

        if self.logging_in {
            self.waiting.tick(width_chunk1[1].width);
            let blk = Block::default()
                .title("Loading")
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
            let (high_user, high_pass) = if self.login.selected == LoginSelect::Username {
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
                &self.login.username,
                Style::default().fg(Color::Cyan),
            )];
            let p = Paragraph::new(t.iter()).block(high_user);

            f.render_widget(p, width_chunk1[1]);

            // Password from here down
            let t2 = [Text::styled(
                "*".repeat(self.login.password.len()),
                Style::default().fg(Color::Cyan),
            )];
            let p2 = Paragraph::new(t2.iter()).block(high_pass);

            f.render_widget(p2, width_chunk2[1])
        }
    }
}
