#![allow(dead_code)]

use std::io::{self};

use std::time::Duration;

use termion::event::{Event as TermEvent, Key, MouseEvent, MouseButton};
use termion::input::MouseTerminal;
use termion::raw::IntoRawMode;

use tui::backend::TermionBackend;
use tui::Terminal;

mod client;
mod ui_loop;
mod widgets;

use ui_loop::{Config, Event, UiEventHandle};
use widgets::app::{AppWidget, DrawWidget};

pub type RoomIdStr = String;
pub type UserIdStr = String;

fn main() -> Result<(), failure::Error> {
    let mut runtime = tokio::runtime::Builder::new()
        .basic_scheduler()
        .threaded_scheduler()
        .enable_all()
        .build()
        .unwrap();

    let executor = runtime.handle().clone();

    runtime.block_on(async {
        let mut app = AppWidget::new(executor).await;
        let events = UiEventHandle::with_config(Config {
            tick_rate: Duration::from_millis(60),
            exit_key: termion::event::Key::Ctrl('q'),
        });
        let stdout = io::stdout().into_raw_mode()?;
        let stdout = MouseTerminal::from(stdout);
        let backend = TermionBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;
        terminal.clear()?;
        terminal.hide_cursor()?;
        loop {
            app.draw(&mut terminal)?;

            if let Some(_er) = app.error.take() {
                while let Event::Tick = events.next()? {}
            }

            match events.next()? {
                Event::Input(event) => match event {
                    TermEvent::Key(key) => match key {
                        Key::Ctrl(c) if c == 'q' => app.should_quit = true,
                        Key::Ctrl(c) if c == 's' => app.on_send().await,
                        Key::Up => app.on_up(),
                        Key::Down => app.on_down(),
                        Key::Backspace => app.on_backspace(),
                        Key::Char(c) => app.on_key(c).await,
                        Key::Esc => app.should_quit = true,
                        _ => {}
                    },
                    TermEvent::Mouse(m) => match m {
                        MouseEvent::Press(btn, x, y) if btn == MouseButton::WheelUp => {
                            app.on_scroll_up(x, y).await
                        },
                        MouseEvent::Press(btn, x, y) => app.on_click(btn, x, y),
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
                app.on_quit().await;
                break;
            }
        }
        Ok(())
    })
}
