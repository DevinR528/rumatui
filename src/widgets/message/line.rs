use tui::style::Style;
use unicode_width::UnicodeWidthStr;

const NBSP: &str = "\u{00a0}";

#[derive(Copy, Clone, Debug)]
pub struct Styled<'a>(pub &'a str, pub Style);

/// A state machine to pack styled symbols into lines.
/// Cannot implement it as Iterator since it yields slices of the internal buffer (need streaming
/// iterators for that).
pub trait LineComposer<'a> {
    fn next_line(&mut self) -> Option<(&[Styled<'a>], u16)>;
}

/// A state machine that wraps lines on word boundaries.
pub struct WordWrapper<'a, 'b> {
    symbols: &'b mut dyn Iterator<Item = Styled<'a>>,
    max_line_width: u16,
    current_line: Vec<Styled<'a>>,
    next_line: Vec<Styled<'a>>,
}

impl<'a, 'b> WordWrapper<'a, 'b> {
    pub fn new(
        symbols: &'b mut dyn Iterator<Item = Styled<'a>>,
        max_line_width: u16,
    ) -> WordWrapper<'a, 'b> {
        WordWrapper {
            symbols,
            max_line_width,
            current_line: vec![],
            next_line: vec![],
        }
    }
}

impl<'a, 'b> LineComposer<'a> for WordWrapper<'a, 'b> {
    fn next_line(&mut self) -> Option<(&[Styled<'a>], u16)> {
        if self.max_line_width == 0 {
            return None;
        }
        std::mem::swap(&mut self.current_line, &mut self.next_line);
        self.next_line.truncate(0);

        let mut current_line_width = self
            .current_line
            .iter()
            .map(|Styled(c, _)| if *c == "\x1b" { 1 } else { c.width() as u16 })
            .sum();

        let mut symbols_to_last_word_end: usize = 0;
        let mut width_to_last_word_end: u16 = 0;
        let mut prev_whitespace = false;
        let mut symbols_exhausted = true;
        for Styled(symbol, style) in &mut self.symbols {
            symbols_exhausted = false;
            let symbol_whitespace = symbol.chars().all(&char::is_whitespace);

            // Ignore characters wider that the total max width.
            if symbol.width() as u16 > self.max_line_width
                // Skip leading whitespace.
                || symbol_whitespace && symbol != "\n" && current_line_width == 0
            {
                continue;
            }

            // Break on newline and discard it.
            if symbol == "\n" {
                if prev_whitespace {
                    current_line_width = width_to_last_word_end;
                    self.current_line.truncate(symbols_to_last_word_end);
                }
                break;
            }

            // Mark the previous symbol as word end.
            if symbol_whitespace && !prev_whitespace && symbol != NBSP {
                symbols_to_last_word_end = self.current_line.len();
                width_to_last_word_end = current_line_width;
            }

            self.current_line.push(Styled(symbol, style));
            current_line_width += symbol.width() as u16;
            if symbol == "\x1b" {
                current_line_width += 1;
            }

            if current_line_width > self.max_line_width {
                // If there was no word break in the text, wrap at the end of the line.
                let (truncate_at, truncated_width) = if symbols_to_last_word_end != 0 {
                    (symbols_to_last_word_end, width_to_last_word_end)
                } else {
                    (self.current_line.len() - 1, self.max_line_width)
                };

                // Push the remainder to the next line but strip leading whitespace:
                {
                    let remainder = &self.current_line[truncate_at..];
                    if let Some(remainder_nonwhite) = remainder
                        .iter()
                        .position(|Styled(c, _)| !c.chars().all(&char::is_whitespace))
                    {
                        self.next_line
                            .extend_from_slice(&remainder[remainder_nonwhite..]);
                    }
                }
                self.current_line.truncate(truncate_at);
                current_line_width = truncated_width;
                break;
            }

            prev_whitespace = symbol_whitespace;
        }

        // Even if the iterator is exhausted, pass the previous remainder.
        if symbols_exhausted && self.current_line.is_empty() {
            None
        } else {
            Some((&self.current_line[..], current_line_width))
        }
    }
}

/// A state machine that truncates overhanging lines.
pub struct LineTruncator<'a, 'b> {
    symbols: &'b mut dyn Iterator<Item = Styled<'a>>,
    max_line_width: u16,
    current_line: Vec<Styled<'a>>,
}

impl<'a, 'b> LineTruncator<'a, 'b> {
    pub fn new(
        symbols: &'b mut dyn Iterator<Item = Styled<'a>>,
        max_line_width: u16,
    ) -> LineTruncator<'a, 'b> {
        LineTruncator {
            symbols,
            max_line_width,
            current_line: vec![],
        }
    }
}

impl<'a, 'b> LineComposer<'a> for LineTruncator<'a, 'b> {
    fn next_line(&mut self) -> Option<(&[Styled<'a>], u16)> {
        if self.max_line_width == 0 {
            return None;
        }

        self.current_line.truncate(0);
        let mut current_line_width = 0;

        let mut skip_rest = false;
        let mut symbols_exhausted = true;
        for Styled(symbol, style) in &mut self.symbols {
            symbols_exhausted = false;

            // Ignore characters wider that the total max width.
            if symbol.width() as u16 > self.max_line_width {
                continue;
            }

            // Break on newline and discard it.
            if symbol == "\n" {
                break;
            }

            if current_line_width + symbol.width() as u16 > self.max_line_width {
                // Exhaust the remainder of the line.
                skip_rest = true;
                break;
            }

            current_line_width += symbol.width() as u16;
            self.current_line.push(Styled(symbol, style));
        }

        if skip_rest {
            for Styled(symbol, _) in &mut self.symbols {
                if symbol == "\n" {
                    break;
                }
            }
        }

        if symbols_exhausted && self.current_line.is_empty() {
            None
        } else {
            Some((&self.current_line[..], current_line_width))
        }
    }
}
