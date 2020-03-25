use std::io;

use tui::backend::Backend;
use tui::layout::{Constraint, Direction, Layout, Rect};
use tui::style::{Color, Modifier, Style};
use tui::widgets::{Block, Borders, Paragraph, Tabs, Text, Widget};
use tui::{Frame, Terminal};

use crate::app::App;

pub fn draw<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<(), io::Error> {
    terminal.draw(|mut f| {
        let chunks = Layout::default()
            .constraints([Constraint::Length(2), Constraint::Min(0)].as_ref())
            .split(f.size());

        Tabs::default()
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(&app.title)
                    .title_style(Style::default().fg(Color::Green).modifier(Modifier::BOLD)),
            )
            .titles(&app.tabs.titles)
            .style(Style::default().fg(Color::Blue))
            .highlight_style(Style::default().fg(Color::Blue).modifier(Modifier::ITALIC))
            .select(app.tabs.index)
            .render(&mut f, chunks[0]);

        draw_app(&mut f, app, chunks[1])
    })
}

fn draw_app<B>(f: &mut Frame<B>, app: &mut App, area: Rect)
where
    B: Backend,
{
    let chunks = Layout::default()
        .constraints([Constraint::Percentage(100)].as_ref())
        .split(area);
    draw_login(f, app, chunks[0]);
}

fn draw_login<B>(f: &mut Frame<B>, app: &mut App, area: Rect)
where
    B: Backend,
{
    let chunks = Layout::default()
        .horizontal_margin(40)
        .constraints([Constraint::Percentage(15), Constraint::Percentage(60), Constraint::Percentage(15),].as_ref())
        .split(area);
    
    Block::default()
        .title("Log In")
        .borders(Borders::ALL)
        .render(f, chunks[1]);

    let height_chunk = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(10), Constraint::Percentage(40), Constraint::Percentage(40), Constraint::Percentage(10),].as_ref())
        .split(chunks[1]);

    let width_chunk1 = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(25), Constraint::Percentage(50), Constraint::Percentage(25),].as_ref())
        .split(height_chunk[1]);
    Paragraph::new([Text::styled("user name", Style::default().fg(Color::Cyan))].iter())
        .block(
            Block::default()
                .title("User Name")
                .borders(Borders::ALL),
        )
        .render(f, width_chunk1[1]);

    let width_chunk2 = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(25), Constraint::Percentage(50), Constraint::Percentage(25),].as_ref())
        .split(height_chunk[2]);

    Paragraph::new([Text::styled("password", Style::default().fg(Color::Cyan))].iter())
        .block(
            Block::default()
                .title("Password")
                .borders(Borders::ALL),
        )
        .render(f, width_chunk2[1])
}
