#![allow(dead_code)]
#![allow(
    clippy::single_component_path_imports,
    clippy::or_fun_call,
    clippy::single_match
)]

use std::{env, io, time::Duration};

use termion::{
    event::{Event as TermEvent, Key, MouseButton, MouseEvent},
    input::MouseTerminal,
    raw::IntoRawMode,
};

use rumatui_tui::{backend::TermionBackend, Terminal};

mod client;
mod error;
mod ui_loop;
mod widgets;

use ui_loop::{Config, Event, UiEventHandle};
use widgets::{app::AppWidget, DrawWidget};

fn main() -> Result<(), failure::Error> {
    // when this is "" empty matrix.org is used
    let server = env::args().nth(1).unwrap_or(String::default());

    let mut runtime = tokio::runtime::Builder::new()
        .basic_scheduler()
        .threaded_scheduler()
        .enable_all()
        .build()
        .unwrap();

    let executor = runtime.handle().clone();

    runtime.block_on(async {
        let mut app = AppWidget::new(executor, &server).await;
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
                    TermEvent::Key(key) => {
                        app.on_notifications().await;

                        match key {
                            Key::Ctrl(c) if c == 'q' => app.should_quit = true,
                            Key::Ctrl(c) if c == 's' => app.on_send().await,
                            Key::Up => app.on_up().await,
                            Key::Down => app.on_down().await,
                            Key::Left => app.on_left(),
                            Key::Right => app.on_right(),
                            Key::Backspace => app.on_backspace(),
                            Key::Delete => app.on_delete().await,
                            Key::Char(c) => app.on_key(c).await,
                            Key::Esc => app.should_quit = true,
                            _ => {}
                        }
                    }
                    TermEvent::Mouse(m) => {
                        app.on_notifications().await;

                        match m {
                            MouseEvent::Press(btn, x, y) if btn == MouseButton::WheelUp => {
                                app.on_scroll_up(x, y).await
                            }
                            MouseEvent::Press(btn, x, y) if btn == MouseButton::WheelDown => {
                                app.on_scroll_down(x, y).await
                            }
                            MouseEvent::Press(btn, x, y) => app.on_click(btn, x, y).await,
                            MouseEvent::Release(_, _) => {}
                            MouseEvent::Hold(_, _) => {}
                        }
                    }
                    TermEvent::Unsupported(_) => {}
                },
                Event::Tick => {
                    app.on_tick(&events).await;
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
