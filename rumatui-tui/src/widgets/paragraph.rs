use std::{cell::Cell, rc::Rc};

use either::Either;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::{
    buffer::Buffer,
    layout::{Alignment, Rect, ScrollMode},
    style::Style,
    widgets::{
        reflow::{LineComposer, LineTruncator, Styled, WordWrapper},
        scroll::{OffsetScroller, ScrolledLine, Scroller, TailScroller},
        Block, Text, Widget,
    },
};

fn get_line_offset(line_width: u16, text_area_width: u16, alignment: Alignment) -> u16 {
    match alignment {
        Alignment::Center => (text_area_width / 2).saturating_sub(line_width / 2),
        Alignment::Right => text_area_width.saturating_sub(line_width),
        Alignment::Left => 0,
    }
}

/// A widget to display some text.
///
/// # Examples
///
/// ```
/// # use rumatui_tui::widgets::{Block, Borders, Paragraph, Text};
/// # use rumatui_tui::style::{Style, Color};
/// # use rumatui_tui::layout::{Alignment};
/// let text = [
///     Text::raw("First line\n"),
///     Text::styled("Second line\n", Style::default().fg(Color::Red))
/// ];
/// Paragraph::new(text.iter())
///     .block(Block::default().title("Paragraph").borders(Borders::ALL))
///     .style(Style::default().fg(Color::White).bg(Color::Black))
///     .alignment(Alignment::Center)
///     .wrap(true);
/// ```
pub struct Paragraph<'a, 't, T>
where
    T: Iterator<Item = &'t Text<'t>>,
{
    /// A block to wrap the widget in
    block: Option<Block<'a>>,
    /// Widget style
    style: Style,
    /// Wrap the text or not
    wrapping: bool,
    /// The text to display
    text: T,
    /// Should we parse the text for embedded commands
    raw: bool,
    /// Scroll offset in number of lines
    scroll: u16,
    /// Indicates if scroll offset starts from top or bottom of content
    scroll_mode: ScrollMode,
    scroll_overflow_char: Option<char>,
    /// Aligenment of the text
    alignment: Alignment,
    /// A flag that is passed in to inform the caller when the buffer
    /// has overflown.
    has_overflown: Option<Rc<Cell<bool>>>,
    at_top: Option<Rc<Cell<bool>>>,
}

impl<'a, 't, T> Paragraph<'a, 't, T>
where
    T: Iterator<Item = &'t Text<'t>>,
{
    pub fn new(text: T) -> Paragraph<'a, 't, T> {
        Paragraph {
            block: None,
            style: Default::default(),
            wrapping: false,
            raw: false,
            text,
            scroll: 0,
            scroll_mode: ScrollMode::Normal,
            scroll_overflow_char: None,
            alignment: Alignment::Left,
            has_overflown: None,
            at_top: None,
        }
    }

    pub fn block(mut self, block: Block<'a>) -> Paragraph<'a, 't, T> {
        self.block = Some(block);
        self
    }

    pub fn style(mut self, style: Style) -> Paragraph<'a, 't, T> {
        self.style = style;
        self
    }

    pub fn wrap(mut self, flag: bool) -> Paragraph<'a, 't, T> {
        self.wrapping = flag;
        self
    }

    pub fn raw(mut self, flag: bool) -> Paragraph<'a, 't, T> {
        self.raw = flag;
        self
    }

    pub fn scroll(mut self, offset: u16) -> Paragraph<'a, 't, T> {
        self.scroll = offset;
        self
    }

    pub fn scroll_mode(mut self, scroll_mode: ScrollMode) -> Paragraph<'a, 't, T> {
        self.scroll_mode = scroll_mode;
        self
    }

    pub fn scroll_overflow_char(
        mut self,
        scroll_overflow_char: Option<char>,
    ) -> Paragraph<'a, 't, T> {
        self.scroll_overflow_char = scroll_overflow_char;
        self
    }

    pub fn alignment(mut self, alignment: Alignment) -> Paragraph<'a, 't, T> {
        self.alignment = alignment;
        self
    }

    pub fn did_overflow(mut self, over: Rc<Cell<bool>>) -> Paragraph<'a, 't, T> {
        self.has_overflown = Some(over);
        self
    }

    pub fn at_top(mut self, top: Rc<Cell<bool>>) -> Paragraph<'a, 't, T> {
        self.at_top = Some(top);
        self
    }
}

impl<'a, 't, 'b, T> Widget for Paragraph<'a, 't, T>
where
    T: Iterator<Item = &'t Text<'t>>,
{
    fn render(mut self, area: Rect, buf: &mut Buffer) {
        let text_area = match self.block {
            Some(ref mut b) => {
                b.render(area, buf);
                b.inner(area)
            }
            None => area,
        };

        if text_area.height < 1 {
            return;
        }

        buf.set_background(text_area, self.style.bg);

        let style = self.style;
        let mut styled = self.text.by_ref().flat_map(|t| match *t {
            Text::Raw(ref d) => {
                let data: &'t str = d; // coerce to &str
                Either::Left(UnicodeSegmentation::graphemes(data, true).map(|g| Styled(g, style)))
            }
            Text::Styled(ref d, s) => {
                let data: &'t str = d; // coerce to &str
                Either::Right(UnicodeSegmentation::graphemes(data, true).map(move |g| Styled(g, s)))
            }
        });

        let line_composer: Box<dyn LineComposer> = if self.wrapping {
            Box::new(WordWrapper::new(&mut styled, text_area.width))
        } else {
            Box::new(LineTruncator::new(&mut styled, text_area.width))
        };

        let mut scrolled_lines: Box<dyn Scroller<'t>> = match self.scroll_mode {
            ScrollMode::Normal => {
                let scroller = OffsetScroller::new(self.scroll, line_composer);
                Box::new(scroller)
            }
            ScrollMode::Tail => {
                let over = self
                    .has_overflown
                    .unwrap_or_else(|| Rc::new(Cell::new(false)));

                let scroller = TailScroller::new(
                    self.scroll,
                    line_composer,
                    text_area.height,
                    Rc::clone(&over),
                );
                Box::new(scroller)
            }
        };

        for y in 0..text_area.height {
            match scrolled_lines.next_line() {
                Some(ScrolledLine::Line(current_line, current_line_width)) => {
                    let mut x =
                        get_line_offset(current_line_width, text_area.width, self.alignment);
                    for Styled(symbol, style) in current_line {
                        buf.get_mut(text_area.left() + x, text_area.top() + y)
                            .set_symbol(symbol)
                            .set_style(style);
                        x += symbol.width() as u16;
                    }
                }
                Some(ScrolledLine::Overflow) => {
                    if let Some(top) = self.at_top.as_ref() {
                        top.set(true);
                    }

                    if let Some(c) = self.scroll_overflow_char {
                        buf.get_mut(text_area.left(), text_area.top() + y)
                            .set_symbol(&c.to_string())
                            .set_style(style);
                    }
                }
                None => {}
            }
        }
    }
}
