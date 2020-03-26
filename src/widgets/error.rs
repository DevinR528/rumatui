use std::io;

use anyhow::{Result, Context, Error};
use tui::backend::Backend;
use tui::layout::{Constraint, Direction, Layout, Rect};
use tui::style::{Color, Modifier, Style};
use tui::widgets::{Block, Borders, Paragraph, Tabs, Text, Widget};
use tui::{Frame, Terminal};

use crate::widgets::RenderWidget;

#[derive(Debug)]
pub struct ErrorWidget<'e>(pub &'e Error);

impl<'e> ErrorWidget<'e> {
    pub fn new(error: &'e Error) -> Self {
        ErrorWidget( error )
    }
}

impl<'e> RenderWidget for ErrorWidget<'e> {
    fn render<B>(&mut self, f: &mut Frame<B>, area: Rect)
    where
        B: Backend,
    {
        let chunks = Layout::default()
            .constraints(
                [
                    Constraint::Percentage(15),
                    Constraint::Percentage(60),
                    Constraint::Percentage(15),
                ]
                .as_ref(),
            )
            .split(f.size());
            
        let chunks = Layout::default()
            .direction(Direction::Horizontal) 
            .constraints(
                [
                    Constraint::Percentage(15),
                    Constraint::Percentage(60),
                    Constraint::Percentage(15),
                ]
                .as_ref(),
            )
            .split(f.size());

        Paragraph::new(
            [Text::styled(
                self.0.to_string(),
                Style::default().fg(Color::Red),
            )]
            .iter(),
        )
            .block(Block::default()
                .title("Error")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Red))
            )
            .render(f, chunks[1])
    }
}
