use anyhow::Error;
use tui::backend::Backend;
use tui::layout::{Constraint, Direction, Layout, Rect};
use tui::style::{Color, Style};
use tui::widgets::{Block, Borders, Paragraph, Text};
use tui::Frame;

use crate::widgets::RenderWidget;

#[derive(Debug)]
pub struct ErrorWidget<'e>(pub &'e Error);

impl<'e> ErrorWidget<'e> {
    pub fn new(error: &'e Error) -> Self {
        ErrorWidget(error)
    }
}

impl<'e> RenderWidget for ErrorWidget<'e> {
    fn render<B>(&mut self, f: &mut Frame<B>, _area: Rect)
    where
        B: Backend,
    {
        let vert_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Constraint::Percentage(25),
                    Constraint::Percentage(50),
                    Constraint::Percentage(25),
                ]
                .as_ref(),
            )
            .split(f.size());

        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(
                [
                    Constraint::Percentage(30),
                    Constraint::Percentage(40),
                    Constraint::Percentage(30),
                ]
                .as_ref(),
            )
            .split(vert_chunks[1]);

        let txt = [Text::styled(
            self.0.to_string(),
            Style::default().fg(Color::Red),
        )];
        let p = Paragraph::new(txt.iter()).block(
            Block::default()
                .title("Error")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Red)),
        );
        f.render_widget(p, chunks[1])
    }
}
