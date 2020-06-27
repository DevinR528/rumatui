use std::fmt;

use muncher::Muncher;
use rumatui_tui::style::{Color, Modifier, Style};
use rumatui_tui::widgets::Text;

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
            // TODO display the wanted text for the link [show_me](http://link.com)
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
            Self {
                ctrl: Vec::new(),
                text: text_or_ctrl,
            }
        }
    }

    pub fn into_text<'a>(self) -> Text<'a> {
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

impl fmt::Display for CtrlChunk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ctrl_code = self
            .ctrl
            .iter()
            .map(|c| {
                if c == "8;;" {
                    format!("\u{1b}]{}", c)
                } else {
                    format!("\u{1b}[{}", c)
                }
            })
            .collect::<String>();
        if ctrl_code.is_empty() && self.text.is_empty() {
            Ok(())
        } else {
            write!(f, "{}{}", ctrl_code, self.text)
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct CtrlChars {
    input: String,
    parsed: Vec<CtrlChunk>,
}

impl fmt::Display for CtrlChars {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = self
            .parsed
            .iter()
            .map(CtrlChunk::to_string)
            .collect::<String>();
        write!(f, "{}", text)
    }
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
        self.parsed.into_iter().map(CtrlChunk::into_text).collect()
    }
}

/// Parses CSI codes and converts them into `Vec<tui::widgets::Text>` chunks.
pub fn process_text<'a>(message: &'a Message) -> Vec<Text<'a>> {
    use itertools::Itertools;

    let name = format!("{}: ", message.name);
    let mut msg = message.text.to_string();
    if msg.contains("    ") {
        msg = msg.replace("    ", "\u{2800}   ");
    }
    let msg = if msg.ends_with('\n') {
        msg
    } else {
        format!("{}\n", msg)
    };

    let body = CtrlChars::parse(msg).into_text();

    let mut formatted = vec![Text::styled(name, Style::default().fg(Color::Magenta))];
    formatted.extend(body);
    // add the reactions
    if !message.reactions.is_empty() {
        let reactions = format!(
            "\u{2800}   {}\n",
            message.reactions.iter().dedup().join(" ")
        );
        formatted.push(Text::raw(reactions));
    }
    formatted
}

// TODO why do all but `failed_message` work locally and fail in travis CI?
#[cfg(test)]
mod test {
    use super::*;
    use mdcat::{self, ResourceAccess, Settings, TerminalCapabilities, TerminalSize};
    use pulldown_cmark::{Options, Parser};
    use std::fmt::{self, Display};
    use std::io::{self, Write};
    use syntect::parsing::SyntaxSet;

    #[derive(Default)]
    pub struct Writer(Vec<u8>);

    impl Write for Writer {
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
    impl Display for Writer {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            if let Ok(s) = String::from_utf8(self.0.clone()) {
                write!(f, "{}", s)
            } else {
                writeln!(f, "to string failed")
            }
        }
    }

    #[test]
    #[ignore] // the ignored tests work perfectly fine locally but fail in CI great x(
    fn test_formatter() {
        let input = "[google](http://www.google.com) `ruma-identifiers` __hello__
# table
- one
- two

```rust
fn main() {
    println!(\"hello\");
}
```";

        let mut options = Options::empty();
        options.insert(Options::ENABLE_TASKLISTS);
        options.insert(Options::ENABLE_STRIKETHROUGH);
        let parser = Parser::new_ext(&input, options);
        let syntax_set = SyntaxSet::load_defaults_nonewlines();

        let settings = Settings {
            terminal_capabilities: TerminalCapabilities::detect(),
            terminal_size: TerminalSize::detect().unwrap(),
            resource_access: ResourceAccess::LocalOnly,
            syntax_set,
        };
        let mut w = Writer::default();
        mdcat::push_tty(&settings, &mut w, &std::path::Path::new("/"), parser).expect("failed");

        let expected = "\u{1b}]8;;http://www.google.com/ \u{1b}[33ruma-identifiers \u{1b}[1hello\n\n\u{1b}[1\u{1b}[34┄\u{1b}[1\u{1b}[34table\n\n• one\n• two\n\n\u{1b}[32────────────────────\n\u{1b}[34fn \u{1b}[33main() {\n    \u{1b}[32println!(\"\u{1b}[36hello\");\n}\n\u{1b}[32────────────────────";

        assert_eq!(expected.trim(), CtrlChars::parse(w.to_string()).to_string())
    }

    #[test]
    #[ignore]
    fn test_formatter2() {
        let input = "[`hi`](http://www.googlelskdnfodaf.com)";

        let mut options = Options::empty();
        options.insert(Options::ENABLE_TASKLISTS);
        options.insert(Options::ENABLE_STRIKETHROUGH);
        let parser = Parser::new_ext(&input, options);
        let syntax_set = SyntaxSet::load_defaults_nonewlines();

        let settings = Settings {
            terminal_capabilities: TerminalCapabilities::detect(),
            terminal_size: TerminalSize::detect().unwrap(),
            resource_access: ResourceAccess::LocalOnly,
            syntax_set,
        };
        let mut w = Writer::default();
        mdcat::push_tty(&settings, &mut w, &std::path::Path::new("/"), parser).expect("failed");

        let ctrl = CtrlChars::parse(w.to_string());
        // println!("{:#?}", ctrl);
        assert_eq!(
            "\u{1b}]8;;http://www.googlelskdnfodaf.com/\n",
            ctrl.to_string(),
        );
    }

    use rumatui_tui::backend::TestBackend;
    use rumatui_tui::layout::Alignment;
    use rumatui_tui::widgets::{Block, Borders, Paragraph};
    use rumatui_tui::Terminal;

    #[test]
    #[ignore]
    fn paragraph_colors() {
        let input = "[google](http://www.google.com) `ruma-identifiers` __hello__
# table
- one
- two

```rust
fn main() {
    println!(\"hello\");
}
```";

        let mut options = Options::empty();
        options.insert(Options::ENABLE_TASKLISTS);
        options.insert(Options::ENABLE_STRIKETHROUGH);
        let parser = Parser::new_ext(&input, options);
        let syntax_set = SyntaxSet::load_defaults_nonewlines();

        let settings = Settings {
            terminal_capabilities: TerminalCapabilities::detect(),
            terminal_size: TerminalSize::detect().unwrap(),
            resource_access: ResourceAccess::LocalOnly,
            syntax_set,
        };
        let mut w = Writer::default();
        mdcat::push_tty(&settings, &mut w, &std::path::Path::new("/"), parser).expect("failed");

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
        let expected = rumatui_tui::buffer::Buffer::with_lines(vec![
            "┌──────────────────┐",
            "│http://www.google.│",
            "│com/              │",
            "│ruma-identifiers  │",
            "│hello             │",
            "│                  │",
            "│┄table            │",
            "│                  │",
            "│• one             │",
            "└──────────────────┘",
        ]);

        // TODO actually check that the colors and formatting are being set this
        // only checks that the symbols (chars) are the same.
        assert!(expected
            .content()
            .iter()
            .zip(render(Alignment::Left).content())
            .all(|(expected, found)| expected.symbol == found.symbol))
    }

    #[test]
    #[ignore]
    fn reply_formatter() {
        let input = "> In reply to blah blah

https://matrix.org/docs/spec/client_server/latest#post-matrix-client-r0-rooms-roomid-leave doesn\'t seem to have a body";

        let mut options = Options::empty();
        options.insert(Options::ENABLE_TASKLISTS);
        options.insert(Options::ENABLE_STRIKETHROUGH);
        let parser = Parser::new_ext(&input, options);
        let syntax_set = SyntaxSet::load_defaults_nonewlines();

        let settings = Settings {
            terminal_capabilities: TerminalCapabilities::detect(),
            terminal_size: TerminalSize::detect().unwrap(),
            resource_access: ResourceAccess::LocalOnly,
            syntax_set,
        };
        let mut w = Writer::default();
        mdcat::push_tty(&settings, &mut w, &std::path::Path::new("/"), parser).expect("failed");

        let expected = "    \u{1b}[3\u{1b}[32In reply to blah blah\n\nhttps://matrix.org/docs/spec/client_server/latest#post-matrix-client-r0-rooms-roomid-leave doesn\'t seem to have a body\n";

        assert_eq!(expected, CtrlChars::parse(w.to_string()).to_string());
        // println!("{:?}", CtrlChars::parse(w.to_string()).to_string())
    }

    #[test]
    #[ignore]
    fn failed_messages() {
        let input = "TWIM: \n# Docker-matrix\n\nThe docker image for synapse v1.12.4rc1 is now on [mvgorcum/docker-matrix:v1.12.4rc1](https://hub.docker.com/r/mvgorcum/docker-matrix/tags)";

        let mut options = Options::empty();
        options.insert(Options::ENABLE_TASKLISTS);
        options.insert(Options::ENABLE_STRIKETHROUGH);
        let parser = Parser::new_ext(&input, options);
        let syntax_set = SyntaxSet::load_defaults_nonewlines();

        let settings = Settings {
            terminal_capabilities: TerminalCapabilities::detect(),
            terminal_size: TerminalSize::detect().unwrap(),
            resource_access: ResourceAccess::LocalOnly,
            syntax_set,
        };
        let mut w = Writer::default();
        mdcat::push_tty(&settings, &mut w, &std::path::Path::new("/"), parser).expect("failed");

        let expected = "TWIM: \n\n\u{1b}[1\u{1b}[34┄\u{1b}[1\u{1b}[34Docker-matrix\n\nThe docker image for synapse v1.12.4rc1 is now on ]8;;https://hub.docker.com/r/mvgorcum/docker-matrix/tags\u{7}\u{1b}[34mvgorcum/docker-matrix:v1.12.4rc1\u{1b}]8;;";
        assert_eq!(expected, CtrlChars::parse(w.to_string()).to_string());
        // println!("{:#?}", CtrlChars::parse(w.to_string()).to_string())
    }
}
