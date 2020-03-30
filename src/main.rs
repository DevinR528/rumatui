#![allow(unused)]
#![allow(dead_code)]

use std::collections::HashMap;
use std::io::{self, Write};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use termion::event::{Event as TermEvent, Key, MouseEvent};
use termion::input::MouseTerminal;
use termion::raw::IntoRawMode;
use tokio::sync::mpsc;
use tokio::runtime::Runtime;
use tui::backend::TermionBackend;
use tui::widgets::Widget;
use tui::Terminal;

mod client;
mod ui_loop;
mod client_loop;
mod widgets;

use widgets::error::ErrorWidget;
use ui_loop::{Config, Event, UiEventHandle};
use widgets::{AppWidget, DrawWidget, RenderWidget};

fn main() -> Result<(), failure::Error> {
    let mut runtime = tokio::runtime::Builder::new()
        .basic_scheduler()
        .threaded_scheduler()
        .enable_all()
        .build()
        .unwrap();

    let executor = runtime.handle().clone();

    runtime.block_on(async {
        let mut app = AppWidget::new(executor);
        let events = UiEventHandle::with_config(Config {
            tick_rate: Duration::from_millis(60),
            exit_key: termion::event::Key::Ctrl('q'),
        });
        let stdout = io::stdout().into_raw_mode()?;
        let stdout = MouseTerminal::from(stdout);
        let backend = TermionBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;
        terminal.clear()?;
        loop {
            app.draw(&mut terminal)?;

            if let Some(er) = app.error.take() {
                println!("SOME ERROR {:?}", er);
                while let Event::Tick = events.next()? {}
            }

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
                    app.on_tick().await;
                }
            }

            if app.should_quit {
                terminal.clear()?;
                break;
            }
        }
        Ok(())
    })
}
