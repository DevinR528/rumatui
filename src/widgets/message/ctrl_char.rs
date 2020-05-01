use muncher::Muncher;
use tui::style::{Color, Modifier, Style};
use tui::widgets::Text;

use super::Message;

#[derive(Clone, Debug, Default)]
pub struct CtrlChunk {
    ctrl: Vec<String>,
    text: String,
}

impl CtrlChunk {
    pub fn text(text: String) -> Self {
        Self {
            ctrl: Vec::new(),
            text,
        }
    }

    pub fn parse(munch: &mut Muncher) -> Self {
        // munch.reset_peek();
        // handles links
        if munch.seek(5) == Some("\u{1b}]8;;".to_string()) {
            let raw_link = munch.eat_until(|c| *c == '\u{7}').collect::<String>();
            // eat all of display text for now
            // TODO display the wanted text for the link
            munch.eat();
            let _ = munch.eat_until(|c| *c == '\u{7}');
            munch.eat();

            let mut link = raw_link.replace("\u{1b}]8;;", "");
            let ws = munch.eat_until(|c| !c.is_whitespace()).collect::<String>();
            link.push_str(&ws);

            return Self {
                ctrl: vec!["8;;".to_string()],
                text: link,
            };
        }

        munch.reset_peek();
        if munch.seek(1) == Some("\u{1b}".to_string()) {
            munch.eat();
        }

        let text_or_ctrl = munch.eat_until(|c| *c == '\u{1b}').collect::<String>();

        if text_or_ctrl.is_empty() {
            return Self {
                ctrl: Vec::new(),
                text: String::new(),
            };
        }

        munch.reset_peek();

        if munch.seek(4) == Some("\u{1b}[0m".to_string()) {
            // eat the reset escape code
            let _ = munch.eat_until(|c| *c == 'm');
            munch.eat();

            let mut ctrl_chars = Vec::new();
            loop {
                let ctrl_text = text_or_ctrl.splitn(2, 'm').collect::<Vec<_>>();

                let mut ctrl = vec![ctrl_text[0].replace("[", "")];
                if ctrl[0].contains(';') {
                    ctrl = ctrl[0].split(';').map(|s| s.to_string()).collect();
                }
                ctrl_chars.extend(ctrl);
                if ctrl_text[1].contains('\u{1b}') {
                    continue;
                } else {
                    let mut text = ctrl_text[1].to_string();

                    let ws = munch.eat_until(|c| !c.is_whitespace()).collect::<String>();
                    text.push_str(&ws);

                    return Self {
                        ctrl: ctrl_chars,
                        text,
                    };
                }
            }
        } else {
            // un control coded text
            return Self {
                ctrl: Vec::new(),
                text: text_or_ctrl,
            };
        }
    }

    pub fn to_text<'a>(self) -> Text<'a> {
        let mut style = Style::default();
        for ctrl in self.ctrl {
            match ctrl {
                // Bold
                ctrl if ctrl == "1" => {
                    style = style.modifier(Modifier::BOLD);
                }
                // Dim/Faint
                ctrl if ctrl == "2" => {
                    style = style.modifier(Modifier::DIM);
                }
                // Italic
                ctrl if ctrl == "3" => {
                    style = style.modifier(Modifier::ITALIC);
                }
                // Underlined
                ctrl if ctrl == "4" => {
                    style = style.modifier(Modifier::UNDERLINED);
                }
                // Slow Blink
                ctrl if ctrl == "5" => {
                    style = style.modifier(Modifier::SLOW_BLINK);
                }
                // Rapid Blink
                ctrl if ctrl == "6" => {
                    style = style.modifier(Modifier::RAPID_BLINK);
                }
                // Reversed
                ctrl if ctrl == "7" => {
                    style = style.modifier(Modifier::REVERSED);
                }
                // Hidden
                ctrl if ctrl == "8" => {
                    style = style.modifier(Modifier::HIDDEN);
                }
                // Crossed Out
                ctrl if ctrl == "9" => {
                    style = style.modifier(Modifier::CROSSED_OUT);
                }
                // Black
                ctrl if ctrl == "30" => {
                    style = style.fg(Color::Black);
                }
                ctrl if ctrl == "40" => {
                    style = style.bg(Color::Black);
                }
                // Red
                ctrl if ctrl == "31" => {
                    style = style.fg(Color::Red);
                }
                ctrl if ctrl == "41" => {
                    style = style.bg(Color::Red);
                }
                // Green
                ctrl if ctrl == "32" => {
                    style = style.fg(Color::Green);
                }
                ctrl if ctrl == "42" => {
                    style = style.bg(Color::Green);
                }
                // Yellow
                ctrl if ctrl == "33" => {
                    style = style.fg(Color::Yellow);
                }
                ctrl if ctrl == "43" => {
                    style = style.bg(Color::Yellow);
                }
                // Blue
                ctrl if ctrl == "34" => {
                    style = style.fg(Color::Blue);
                }
                ctrl if ctrl == "44" => {
                    style = style.bg(Color::Blue);
                }
                // Magenta
                ctrl if ctrl == "35" => {
                    style = style.fg(Color::Magenta);
                }
                ctrl if ctrl == "45" => {
                    style = style.bg(Color::Magenta);
                }
                // Cyan
                ctrl if ctrl == "36" => {
                    style = style.fg(Color::Cyan);
                }
                ctrl if ctrl == "46" => {
                    style = style.bg(Color::Cyan);
                }
                // White
                ctrl if ctrl == "37" => {
                    style = style.fg(Color::White);
                }
                ctrl if ctrl == "47" => {
                    style = style.bg(Color::White);
                }
                // Bright Colors
                // Black
                ctrl if ctrl == "90" => {
                    style = style.fg(Color::DarkGray);
                }
                ctrl if ctrl == "100" => {
                    style = style.bg(Color::DarkGray);
                }
                // Red
                ctrl if ctrl == "91" => {
                    style = style.fg(Color::LightRed);
                }
                ctrl if ctrl == "101" => {
                    style = style.bg(Color::LightRed);
                }
                // Green
                ctrl if ctrl == "92" => {
                    style = style.fg(Color::LightGreen);
                }
                ctrl if ctrl == "102" => {
                    style = style.bg(Color::LightGreen);
                }
                // Yellow
                ctrl if ctrl == "93" => {
                    style = style.fg(Color::LightYellow);
                }
                ctrl if ctrl == "103" => {
                    style = style.bg(Color::LightYellow);
                }
                // Blue
                ctrl if ctrl == "94" => {
                    style = style.fg(Color::LightBlue);
                }
                ctrl if ctrl == "104" => {
                    style = style.bg(Color::LightBlue);
                }
                // Magenta
                ctrl if ctrl == "95" => {
                    style = style.fg(Color::LightMagenta);
                }
                ctrl if ctrl == "105" => {
                    style = style.bg(Color::LightMagenta);
                }
                // Cyan
                ctrl if ctrl == "96" => {
                    style = style.fg(Color::LightCyan);
                }
                ctrl if ctrl == "106" => {
                    style = style.bg(Color::LightCyan);
                }
                // tui has no "Bright White" color code equivalent
                // White
                ctrl if ctrl == "97" => {
                    style = style.fg(Color::White);
                }
                ctrl if ctrl == "107" => {
                    style = style.bg(Color::White);
                }
                // _ => panic!("control sequence not found"),
                _ => return Text::raw(self.text),
            };
        }
        Text::styled(self.text, style)
    }
}

#[derive(Clone, Debug, Default)]
pub struct CtrlChars {
    input: String,
    parsed: Vec<CtrlChunk>,
}

impl CtrlChars {
    pub fn parse(input: String) -> Self {
        let mut parsed = Vec::new();

        let mut munch = Muncher::new(&input);
        let pre_ctrl = munch.eat_until(|c| *c == '\u{1b}').collect::<String>();
        parsed.push(CtrlChunk::text(pre_ctrl));

        loop {
            if munch.is_done() {
                break;
            } else {
                parsed.push(CtrlChunk::parse(&mut munch))
            }
        }
        Self {
            input: input.to_string(),
            parsed,
        }
    }

    pub fn into_text<'a>(self) -> Vec<Text<'a>> {
        self.parsed.into_iter().map(CtrlChunk::to_text).collect()
    }
}

/// Parses CSI codes and converts them into `Vec<tui::widgets::Text>` chunks.
pub fn process_text<'a>(msg: &'a Message) -> Vec<Text<'a>> {
    let name = format!("{}: ", msg.name);
    let mut msg = msg.text.to_string();
    if msg.contains("    ") {
        msg = msg.replace("    ", "\u{2800}   ");
    }
    let msg = if msg.ends_with('\n') {
        msg.to_string()
    } else {
        format!("{}\n", msg)
    };

    let body = CtrlChars::parse(msg).into_text();

    let mut formatted = vec![Text::styled(name, Style::default().fg(Color::Magenta))];
    formatted.extend(body);

    formatted
}

#[cfg(test)]
mod test {
    use super::*;
    use mdcat::{self, ResourceAccess, TerminalCapabilities, TerminalSize};
    use pulldown_cmark::{Options, Parser};
    use std::fmt::{self, Display};
    use std::io::{self, Write};
    use syntect::parsing::SyntaxSet;

    #[derive(Default)]
    pub struct Writter(Vec<u8>);

    impl Write for Writter {
        #[inline]
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.extend_from_slice(buf);
            Ok(buf.len())
        }
        #[inline]
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    impl Display for Writter {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            if let Ok(s) = String::from_utf8(self.0.clone()) {
                write!(f, "{}", s)
            } else {
                writeln!(f, "to string failed")
            }
        }
    }

    #[test]
    fn test_formatter() {
        let input = r#"[google](http://www.google.com) `ruma-identifiers` __hello__
# table
- one
- two

```rust
fn main() {
    println!("hello");
}
```"#;

        let mut options = Options::empty();
        options.insert(Options::ENABLE_TASKLISTS);
        options.insert(Options::ENABLE_STRIKETHROUGH);
        let parser = Parser::new_ext(&input, options);
        let syntax_set = SyntaxSet::load_defaults_nonewlines();

        let mut w = Writter::default();
        mdcat::push_tty(
            &mut w,
            &TerminalCapabilities::detect(),
            TerminalSize::detect().unwrap(),
            parser,
            &std::path::Path::new("/"),
            ResourceAccess::RemoteAllowed,
            syntax_set,
        )
        .expect("failed");

        println!("{:?}\n\n", w.to_string());
        CtrlChars::parse(w.to_string()).into_text();
    }

    #[test]
    fn test_formatter2() {
        let input = r#"[`hi`](http://www.googlelskdnfodaf.com)"#;

        let mut options = Options::empty();
        options.insert(Options::ENABLE_TASKLISTS);
        options.insert(Options::ENABLE_STRIKETHROUGH);
        let parser = Parser::new_ext(&input, options);
        let syntax_set = SyntaxSet::load_defaults_nonewlines();

        let mut w = Writter::default();
        mdcat::push_tty(
            &mut w,
            &TerminalCapabilities::detect(),
            TerminalSize::detect().unwrap(),
            parser,
            &std::path::Path::new("/"),
            ResourceAccess::RemoteAllowed,
            syntax_set,
        )
        .expect("failed");

        println!("{:?}\n\n", w.to_string());
        println!("{:#?}", CtrlChars::parse(w.to_string()).into_text());
    }

    use tui::backend::TestBackend;
    use tui::layout::Alignment;
    use tui::widgets::{Block, Borders, Paragraph};
    use tui::Terminal;

    #[test]
    fn paragraph_colors() {
        let input = r#"[google](http://www.google.com) `ruma-identifiers` __hello__
# table
- one
- two

```rust
fn main() {
    println!("hello");
}
```"#;

        let mut options = Options::empty();
        options.insert(Options::ENABLE_TASKLISTS);
        options.insert(Options::ENABLE_STRIKETHROUGH);
        let parser = Parser::new_ext(&input, options);
        let syntax_set = SyntaxSet::load_defaults_nonewlines();

        let mut w = Writter::default();
        mdcat::push_tty(
            &mut w,
            &TerminalCapabilities::detect(),
            TerminalSize::detect().unwrap(),
            parser,
            &std::path::Path::new("/"),
            ResourceAccess::RemoteAllowed,
            syntax_set,
        )
        .expect("failed");

        let text = CtrlChars::parse(w.to_string()).into_text();

        let render = |alignment| {
            let backend = TestBackend::new(20, 10);
            let mut terminal = Terminal::new(backend).unwrap();

            terminal
                .draw(|mut f| {
                    let size = f.size();
                    let paragraph = Paragraph::new(text.iter())
                        .block(Block::default().borders(Borders::ALL))
                        .alignment(alignment)
                        .wrap(true);
                    f.render_widget(paragraph, size);
                })
                .unwrap();
            terminal.backend().buffer().clone()
        };

        println!("{:#?}", render(Alignment::Left))
    }

    #[test]
    fn reply_formatter() {
        let input = r#"> In reply to blah blah

https://matrix.org/docs/spec/client_server/latest#post-matrix-client-r0-rooms-roomid-leave doesn\'t seem to have a body"#;

        let mut options = Options::empty();
        options.insert(Options::ENABLE_TASKLISTS);
        options.insert(Options::ENABLE_STRIKETHROUGH);
        let parser = Parser::new_ext(&input, options);
        let syntax_set = SyntaxSet::load_defaults_nonewlines();

        let mut w = Writter::default();
        mdcat::push_tty(
            &mut w,
            &TerminalCapabilities::detect(),
            TerminalSize::detect().unwrap(),
            parser,
            &std::path::Path::new("/"),
            ResourceAccess::RemoteAllowed,
            syntax_set,
        )
        .expect("failed");

        let parsed = CtrlChars::parse(w.to_string());

        println!("{:#?}", parsed);
        let _text = parsed.into_text();
    }

    #[test]
    fn failed_messages() {
        let input = r#"TWIM: \n# Docker-matrix\n\nThe docker image for synapse v1.12.4rc1 is now on [mvgorcum/docker-matrix:v1.12.4rc1](https://hub.docker.com/r/mvgorcum/docker-matrix/tags)"#;

        let mut options = Options::empty();
        options.insert(Options::ENABLE_TASKLISTS);
        options.insert(Options::ENABLE_STRIKETHROUGH);
        let parser = Parser::new_ext(&input, options);
        let syntax_set = SyntaxSet::load_defaults_nonewlines();

        let mut w = Writter::default();
        mdcat::push_tty(
            &mut w,
            &TerminalCapabilities::detect(),
            TerminalSize::detect().unwrap(),
            parser,
            &std::path::Path::new("/"),
            ResourceAccess::RemoteAllowed,
            syntax_set,
        )
        .expect("failed");

        let parsed = CtrlChars::parse(w.to_string());
        print!("{:?}", parsed);
    }
}
