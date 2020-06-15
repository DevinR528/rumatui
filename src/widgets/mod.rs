use std::io;

use rumatui_tui::{backend::Backend, layout::Rect, Frame, Terminal};

pub mod app;
pub mod chat;
mod error;
pub mod login;
pub mod message;
pub mod register;
pub mod room_search;
pub mod rooms;
pub mod utils;

pub trait RenderWidget {
    fn render<B>(&mut self, f: &mut Frame<B>, area: Rect)
    where
        B: Backend;
}

pub trait DrawWidget {
    fn draw<B>(&mut self, terminal: &mut Terminal<B>) -> io::Result<()>
    where
        B: Backend + Send;
    fn draw_with<B>(&mut self, _terminal: &mut Terminal<B>, _area: Rect) -> io::Result<()>
    where
        B: Backend,
    {
        Ok(())
    }
}
