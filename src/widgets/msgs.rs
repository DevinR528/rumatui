

use tui::backend::Backend;
use tui::layout::{Constraint, Direction, Layout, Rect};
use tui::style::{Color, Modifier, Style};
use tui::widgets::{Block, Borders, Paragraph, Text, Widget};
use tui::{Frame};

use crate::widgets::RenderWidget;

#[derive(Clone, Debug, Default)]
pub struct MessageWidget {
    messages: Vec<String>,
}

impl MessageWidget {
    pub fn add_message(&mut self, msg: String) {
        self.messages.push(msg)
    }
}

impl RenderWidget for MessageWidget {
    fn render<B>(&mut self, f: &mut Frame<B>, area: Rect)
    where
        B: Backend,
    {
        let _chunks = Layout::default()
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
            .direction(Direction::Horizontal)
            .split(area);

        let text = self.messages.join("\n");
            Paragraph::new(
                vec![Text::styled(
                    text,
                    Style::default().fg(Color::Blue),
                )]
                .iter(),
            )
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Green).modifier(Modifier::BOLD))
                    .title("Incoming")
                    .title_style(
                        Style::default()
                            .fg(Color::Yellow)
                            .modifier(Modifier::BOLD),
                    ),
            )
            .wrap(true)
            .render(f, area);
    }
}
