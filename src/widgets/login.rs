use std::io;

use tui::backend::Backend;
use tui::layout::{Constraint, Direction, Layout, Rect};
use tui::style::{Color, Modifier, Style};
use tui::widgets::{Block, Borders, Paragraph, Tabs, Text, Widget};
use tui::{Frame, Terminal};

use crate::widgets::RenderWidget;

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
    pub login: Login,
    pub logged_in: bool,
}

impl LoginWidget {}

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

        Block::default()
            .title("Log In")
            .borders(Borders::ALL)
            .render(f, chunks[1]);

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
        Paragraph::new(
            [Text::styled(
                &self.login.username,
                Style::default().fg(Color::Cyan),
            )]
            .iter(),
        )
        .block(high_user)
        .render(f, width_chunk1[1]);

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

        Paragraph::new(
            [Text::styled(
                &self.login.password,
                Style::default().fg(Color::Cyan),
            )]
            .iter(),
        )
        .block(high_pass)
        .render(f, width_chunk2[1])
    }
}
