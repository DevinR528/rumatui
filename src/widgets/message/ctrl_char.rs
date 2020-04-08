use muncher::Muncher;
use tui::widgets::{Block, Borders, Paragraph, Text};
use tui::style::{Color, Modifier, Style};

#[derive(Debug, Default)]
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
        munch.eat();
        let text_or_ctrl = munch.eat_until(|c| *c == '\u{1b}').collect::<String>();
        if munch.seek(3) == Some("[0m".to_string()) {
            println!("{:?}", text_or_ctrl);
        } else {
            println!("{:?}", text_or_ctrl);
        }

        Self {
            ctrl: Vec::new(),
            text: String::new(),
        }
    }
}

#[derive(Debug, Default)]
pub struct CtrlChars {
    input: String,
    parsed: Vec<CtrlChunk>,
}

impl CtrlChars {
    pub fn parse(input: &str) -> Self {
        let mut parsed = Vec::new();

        let mut munch = Muncher::new(input);
        let pre_ctrl = munch.eat_until(|c| *c == '\u{1b}').collect::<String>();
        parsed.push(CtrlChunk::text(pre_ctrl));

        loop {
            if munch.is_done() {
                break;
            } else {
                parsed.push(CtrlChunk::parse(&mut munch))
            }
        }
        Self { input: input.to_string(), parsed, }
    }
}

fn format_message_body<'a>(input: String) -> Vec<Text<'a>> {
    let matches = CtrlChars::parse(&input);
    vec![ Text::styled(input.to_string(), Style::default().fg(Color::Cyan)) ]
}

pub fn process_text<'a>(msg: &'a str) -> Vec<Text<'a>> {
    let msg = format!("{}\n", msg);
    let split = msg.splitn(2, " ").collect::<Vec<_>>();

    let name = format!("{} ", split[0]);
    let body = format_message_body(split[1].to_string());

    let mut formatted = vec![ Text::styled(name, Style::default().fg(Color::Magenta)) ];
    formatted.extend(body);

    formatted
} 


#[cfg(test)]
mod test {
    use super::*;
    use std::io::{self, Write};
    use std::fmt::{self, Display};
    use pulldown_cmark::{Options, Parser};
    use syntect::parsing::SyntaxSet;
    use mdcat::{self, ResourceAccess, TerminalCapabilities, TerminalSize};

    #[derive(Default)]
    pub struct Writter(Vec<u8>);

    impl Write for Writter {
        #[inline]
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.extend_from_slice(buf);
            Ok(buf.len())
        }
        #[inline]
        fn flush(&mut self) -> io::Result<()> { Ok(()) }
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
        format_message_body(w.to_string());
    }
}
