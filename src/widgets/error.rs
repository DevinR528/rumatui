use rumatui_tui::backend::Backend;
use rumatui_tui::layout::{Constraint, Direction, Layout, Rect};
use rumatui_tui::style::{Color, Style};
use rumatui_tui::widgets::{Block, Borders, Paragraph, Text};
use rumatui_tui::Frame;

use crate::{error::Error, widgets::RenderWidget};

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
        let p = Paragraph::new(txt.iter())
            .block(
                Block::default()
                    .title("Error")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Red)),
            )
            .wrap(true);
        f.render_widget(p, chunks[1])
    }
}
