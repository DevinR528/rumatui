use std::io;

use tui::backend::Backend;
use tui::layout::{Constraint, Direction, Layout, Rect};
use tui::style::{Color, Modifier, Style};
use tui::widgets::{Block, Borders, Paragraph, Tabs, Text, Widget};
use tui::{Frame, Terminal};

use crate::widgets::RenderWidget;

#[derive(Clone, Debug, Default)]
pub struct MessageWidget {
    pub room: Vec<String>,
}

impl MessageWidget {}

impl RenderWidget for MessageWidget {
    fn render<B>(&mut self, f: &mut Frame<B>, area: Rect)
    where
        B: Backend,
    {
    }
}
