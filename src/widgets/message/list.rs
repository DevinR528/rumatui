use std::iter::{self, Iterator};

use either::Either;
use unicode_width::{UnicodeWidthStr};
use unicode_segmentation::UnicodeSegmentation;

use tui::buffer::Buffer;
use tui::layout::{Alignment,Corner, Rect};
use tui::style::Style;
use tui::widgets::{Block, StatefulWidget, Text, Widget};

use super::line::{LineComposer, LineTruncator, Styled, WordWrapper};

fn get_line_offset(line_width: u16, text_area_width: u16, alignment: Alignment) -> u16 {
    match alignment {
        Alignment::Center => (text_area_width / 2).saturating_sub(line_width / 2),
        Alignment::Right => text_area_width.saturating_sub(line_width),
        Alignment::Left => 0,
    }
}

pub struct ListState {
    offset: usize,
    selected: Option<usize>,
}

impl Default for ListState {
    fn default() -> ListState {
        ListState {
            offset: 0,
            selected: None,
        }
    }
}

impl ListState {
    pub fn selected(&self) -> Option<usize> {
        self.selected
    }

    pub fn select(&mut self, index: Option<usize>) {
        self.selected = index;
        if index.is_none() {
            self.offset = 0;
        }
    }
}

/// A widget to display several items among which one can be selected (optional)
///
/// # Examples
///
/// ```
/// # use tui::widgets::{Block, Borders, List, Text};
/// # use tui::style::{Style, Color, Modifier};
/// let items = ["Item 1", "Item 2", "Item 3"].iter().map(|i| Text::raw(*i));
/// List::new(items)
///     .block(Block::default().title("List").borders(Borders::ALL))
///     .style(Style::default().fg(Color::White))
///     .highlight_style(Style::default().modifier(Modifier::ITALIC))
///     .highlight_symbol(">>");
/// ```
pub struct List<'b> {
    block: Option<Block<'b>>,
    items: Vec<Text<'b>>,
    start_corner: Corner,
    /// Base style of the widget
    style: Style,
    /// Does this widget wrap the given text.
    wrap: bool,
    /// Scroll
    scroll: u16,
    /// Style used to render selected item
    highlight_style: Style,
    /// Symbol in front of the selected item (Shift all items to the right)
    highlight_symbol: Option<&'b str>,
    /// Aligenment of the text
    alignment: Alignment,
}

impl<'b> Default for List<'b> {
    fn default() -> List<'b> {
        List {
            block: None,
            items: Vec::default(),
            style: Default::default(),
            wrap: false,
            scroll: 0,
            start_corner: Corner::TopLeft,
            highlight_style: Style::default(),
            highlight_symbol: None,
            alignment: Alignment::Left,
        }
    }
}

impl<'b> List<'b> {
    pub fn new(items: Vec<Text<'b>>) -> List<'b> {
        List {
            block: None,
            items,
            style: Default::default(),
            wrap: false,
            scroll: 0,
            start_corner: Corner::TopLeft,
            highlight_style: Style::default(),
            highlight_symbol: None,
            alignment: Alignment::Left,
        }
    }

    pub fn block(mut self, block: Block<'b>) -> List<'b> {
        self.block = Some(block);
        self
    }

    pub fn items<I>(mut self, items: Vec<Text<'b>>) -> List<'b> {
        self.items = items;
        self
    }

    pub fn style(mut self, style: Style) -> List<'b> {
        self.style = style;
        self
    }

    pub fn wrap(mut self, wrap: bool) -> List<'b> {
        self.wrap = wrap;
        self
    }

    pub fn highlight_symbol(mut self, highlight_symbol: &'b str) -> List<'b> {
        self.highlight_symbol = Some(highlight_symbol);
        self
    }

    pub fn highlight_style(mut self, highlight_style: Style) -> List<'b> {
        self.highlight_style = highlight_style;
        self
    }

    pub fn start_corner(mut self, corner: Corner) -> List<'b> {
        self.start_corner = corner;
        self
    }
}

impl<'b> tui::widgets::Widget for List<'b> {
    fn render(mut self, area: Rect, buf: &mut Buffer) {
        let text_area = match self.block {
            Some(ref mut b) => {
                b.render(area, buf);
                b.inner(area)
            }
            None => area,
        };

        if text_area.width < 1 || text_area.height < 1 {
            return;
        }

        let list_height = text_area.height as usize;

        buf.set_background(text_area, self.style.bg);
        // TODO is this as cheap as can be done
        let above_border = self.items.len().saturating_sub(list_height);
        let style = self.style;

        let mut y = 0;
        for (i, text) in self.items
            .iter()
            .skip(above_border)
            .enumerate()
        {
            let mut styled = match text {
                Text::Raw(ref d) => {
                    let data: &str = d; // coerce to &str
                    Either::Left(UnicodeSegmentation::graphemes(data, true).map(|g| Styled(g, style)))
                }
                Text::Styled(ref d, s) => {
                    let data: &str = d; // coerce to &str
                    Either::Right(UnicodeSegmentation::graphemes(data, true).map(move |g| Styled(g, *s)))
                }
            };
    
            let mut line_composer: Box<dyn LineComposer> = if self.wrap {
                Box::new(WordWrapper::new(&mut styled, text_area.width))
            } else {
                Box::new(LineTruncator::new(&mut styled, text_area.width))
            };
            let mut line_split = 0;
            while let Some((current_line, current_line_width)) = line_composer.next_line() {
                if y >= self.scroll {
                    let mut x = get_line_offset(current_line_width, text_area.width, self.alignment);
                    if line_split > 0 {
                        x += 0;
                    }
                    for Styled(symbol, style) in current_line {
                        buf.get_mut(text_area.left() + x, text_area.top() + y - self.scroll)
                            .set_symbol(symbol)
                            .set_style(*style);
                        x += symbol.width() as u16;
                        if *symbol == "\x1b" {
                            x += 1;
                        }
                    }
                }
                y += 1;
                if y >= text_area.height + self.scroll {
                    break;
                }
                line_split += 1;
            }
            if y >= text_area.height + self.scroll {
                break;
            }
        }
    }
}
