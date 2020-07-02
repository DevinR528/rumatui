#![allow(dead_code)]
#![allow(
    clippy::single_component_path_imports,
    clippy::or_fun_call,
    clippy::single_match
)]

use std::{env, io, process, time::Duration};

use termion::{
    event::{Event as TermEvent, Key, MouseButton, MouseEvent},
    input::MouseTerminal,
    raw::IntoRawMode,
};

use rumatui_tui::{backend::TermionBackend, Terminal};

mod client;
mod error;
mod log;
mod ui_loop;
mod widgets;

use ui_loop::{Config, Event, UiEventHandle};
use widgets::{app::AppWidget, DrawWidget};

const VERSION: &str = env!("CARGO_PKG_VERSION");

lazy_static::lazy_static! {
    pub static ref RUMATUI_DIR: std::io::Result<std::path::PathBuf> = {
        let mut path = dirs::home_dir()
            .ok_or(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "no home directory found",
            ))?;
        path.push(".rumatui");
        Ok(path)
    };
}

fn main() -> Result<(), failure::Error> {
    // when this is "" empty matrix.org is used
    let server = if let Some(arg) = env::args().nth(1) {
        if arg.contains("help") || arg == "-h" {
            print_help();
            process::exit(0)
        } else {
            // we assume this is a server address
            arg
        }
    } else {
        String::new()
    };

    let mut runtime = tokio::runtime::Builder::new()
        .basic_scheduler()
        .threaded_scheduler()
        .enable_all()
        .build()
        .unwrap();

    let path: &std::path::Path = RUMATUI_DIR
        .as_ref()
        .map_err(|e| failure::format_err!("home dir not found: {}", e))?;
    let mut path = std::path::PathBuf::from(path);
    path.push("logs.json");

    let exec = runtime.handle().clone();
    let (logger, _log_handle) = log::Logger::spawn_logger(&path, exec)?;

    tracing_subscriber::fmt()
        .with_writer(logger)
        .json()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    // .try_init()
    // .unwrap(); // they return `<dyn Error + Send + Sync + 'static>`

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
                            Key::Ctrl(c) if c == 'd' => app.on_ctrl_d().await,
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

#[rustfmt::skip]
#[allow(clippy::print_literal)]
fn print_help() {
    println!(
        "rumatui {} \n\n{}{}{}{}{}{}",
        VERSION,
        "USAGE:\n",
        "   rumatui [HOMESERVER]\n\n",
        "OPTIONS:\n",
        "   -h, --help      Prints help information\n\n",
        "KEY-BINDINGS:",
r#"
    * Esc will exit `rumatui`
    * Enter still works for all buttons except the decline/accept invite
    * Ctrl-s sends a message
    * Delete leaves and forgets the selected room
    * Left/right arrows, while at the login window, toggles login/register window
    * Left arrow, while at the main chat window, brings up the room search window
    * Enter, while in the room search window, starts the search
    * Ctrl-d, while a room is selected in the room search window, joins the room
"#,
    )
}
