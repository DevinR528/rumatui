use std::{
    io,
    sync::mpsc,
    thread,
    time::Duration,
};

use termion::{
    event::{Event as TermEvent, Key},
    input::{MouseTerminal, TermRead},
    raw::IntoRawMode,
};

pub enum Event<I> {
    Input(I),
    Tick,
}

/// A small event handler that wrap termion input and tick events. Each event
/// type is handled in its own thread and returned to a common `Receiver`
pub struct UiEventHandle {
    recv: mpsc::Receiver<Event<TermEvent>>,
    input_handle: thread::JoinHandle<()>,
    tick_handle: thread::JoinHandle<()>,
}

#[derive(Debug, Clone, Copy)]
pub struct Config {
    pub exit_key: Key,
    pub tick_rate: Duration,
}

impl UiEventHandle {
    pub fn with_config(cfg: Config) -> Self {
        let (send, recv) = mpsc::channel();

        let stdout = io::stdout().into_raw_mode().unwrap();
        let _stdout = MouseTerminal::from(stdout);

        let input_handle = {
            let send = send.clone();
            thread::spawn(move || {
                let stdin = io::stdin();
                for ev in stdin.events() {
                    let ev = ev.unwrap();

                    if let TermEvent::Key(Key::Char('q')) = ev {
                        return;
                    }

                    if send.send(Event::Input(ev)).is_err() {
                        return;
                    }
                }
            })
        };
        let tick_handle = {
            thread::spawn(move || loop {
                if let Err(_e) = send.send(Event::Tick) {
                    return;
                }
                thread::sleep(cfg.tick_rate);
            })
        };

        UiEventHandle {
            recv,
            input_handle,
            tick_handle,
        }
    }

    pub fn next(&self) -> Result<Event<TermEvent>, mpsc::RecvError> {
        self.recv.recv()
    }

    #[allow(dead_code)]
    pub fn shutdown(self) {
        let _ = self.input_handle.join();
        let _ = self.tick_handle.join();
    }
}
