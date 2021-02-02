// All credit goes to https://github.com/clevinson/tui-rs. His fork and work
// on scrolling is represented here with a few changes.

use std::{cell::Cell, rc::Rc};

use crate::widgets::reflow::{LineComposer, Styled};

pub trait Scroller<'t> {
    fn next_line(&mut self) -> Option<ScrolledLine<'t>>;
}

pub enum ScrolledLine<'t> {
    Overflow,
    Line(Vec<Styled<'t>>, u16),
}

pub struct OffsetScroller<'t, 'lc> {
    next_line_offset: u16,
    line_composer: Box<dyn LineComposer<'t> + 'lc>,
}

impl<'t, 'lc> OffsetScroller<'t, 'lc> {
    pub fn new(
        scroll_offset: u16,
        line_composer: Box<dyn LineComposer<'t> + 'lc>,
    ) -> OffsetScroller<'t, 'lc> {
        OffsetScroller {
            next_line_offset: scroll_offset,
            line_composer,
        }
    }
}

impl<'t, 'lc> Scroller<'t> for OffsetScroller<'t, 'lc> {
    fn next_line(&mut self) -> Option<ScrolledLine<'t>> {
        if self.next_line_offset > 0 {
            for _ in 0..self.next_line_offset {
                self.line_composer.next_line();
            }
            self.next_line_offset = 0;
        }
        self.line_composer
            .next_line()
            .map(|(line, line_width)| ScrolledLine::Line(line.to_vec(), line_width))
            .or(Some(ScrolledLine::Overflow))
    }
}

pub struct TailScroller<'t> {
    next_line_offset: i16,
    all_lines: Vec<(Vec<Styled<'t>>, u16)>,
}

impl<'t, 'lc> TailScroller<'t> {
    pub fn new(
        scroll_offset: u16,
        mut line_composer: Box<dyn LineComposer<'t> + 'lc>,
        text_area_height: u16,
        has_overflown: Rc<Cell<bool>>,
    ) -> TailScroller<'t> {
        let mut all_lines = line_composer.collect_lines();
        all_lines.reverse();
        let num_lines = all_lines.len() as u16;

        // scrolling up back in history past the top of the content results
        // in a ScrollLine::Overflow, so as to allow for the renderer to
        // draw a special scroll_overflow_char on each subsequent line
        let next_line_offset = if num_lines <= text_area_height {
            if num_lines + 2 >= text_area_height {
                has_overflown.set(true);
            }
            // if content doesn't fill the text_area_height,
            // scrolling should be reverse of normal
            // behavior
            -(scroll_offset as i16)
        } else {
            has_overflown.set(true);
            // default ScrollFrom::Bottom behavior,
            // scroll == 0 floats content to bottom,
            // scroll > 0 scrolling up, back in history
            num_lines as i16 - (text_area_height + scroll_offset) as i16
        };

        TailScroller {
            next_line_offset,
            all_lines,
        }
    }
}

impl<'t> Scroller<'t> for TailScroller<'t> {
    fn next_line(&mut self) -> Option<ScrolledLine<'t>> {
        if self.next_line_offset < 0 {
            self.next_line_offset += 1;
            Some(ScrolledLine::Overflow)
        } else {
            if self.next_line_offset > 0 {
                for _ in 0..self.next_line_offset {
                    self.all_lines.pop();
                }
                self.next_line_offset = 0;
            }
            self.all_lines
                .pop()
                .map(|(line, line_width)| ScrolledLine::Line(line, line_width))
        }
    }
}
