#![allow(unused)]
#![allow(dead_code)]

use std::collections::HashMap;
use std::io::{self, Write};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use termion::event::{Event as TermEvent, Key, MouseEvent};
use termion::input::MouseTerminal;
use termion::raw::IntoRawMode;
use tui::backend::TermionBackend;
use tui::widgets::Widget;
use tui::Terminal;

mod client;
mod ev_loop;
mod widgets;

use ev_loop::{Config, Event, EventHandle};
use widgets::{AppWidget, DrawWidget, RenderWidget};

#[tokio::main]
async fn main() -> Result<(), failure::Error> {
    let mut args = std::env::args();
    let tick_rate = if let Some(tick) = args.find(|arg| arg.parse::<u64>().is_ok()) {
        tick.parse()?
    } else {
        250
    };

    let mut app = AppWidget::new().await.expect("error from `forget`");

    let events = EventHandle::with_config(Config {
        tick_rate: Duration::from_millis(tick_rate),
        exit_key: termion::event::Key::Ctrl('q'),
    });

    let stdout = io::stdout().into_raw_mode()?;
    let stdout = MouseTerminal::from(stdout);
    let backend = TermionBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    terminal.clear()?;
    loop {
        app.draw(&mut terminal);
        match events.next()? {
            Event::Input(event) => match event {
                TermEvent::Key(key) => match key {
                    Key::Ctrl(c) if c == 'q' => app.should_quit = true,
                    Key::Up => app.on_up(),
                    Key::Down => app.on_down(),
                    Key::Backspace => app.on_backspace(),
                    Key::Char(c) => app.on_key(c).await,
                    Key::Esc => app.should_quit = true,
                    _ => {}
                },
                TermEvent::Mouse(m) => match m {
                    MouseEvent::Press(_button, x, y) => {
                        terminal.set_cursor(x, y).unwrap();
                    }
                    MouseEvent::Release(_, _) => {}
                    MouseEvent::Hold(_, _) => {}
                },
                TermEvent::Unsupported(_) => {}
            },
            Event::Tick => {
                app.on_tick();
            }
        }
        if app.should_quit {
            terminal.clear()?;
            break;
        }
    }

    Ok(())
}
