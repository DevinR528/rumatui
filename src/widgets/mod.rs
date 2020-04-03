pub mod app;
pub mod chat;
pub mod error;
mod login;
mod msgs;
mod rooms;
mod utils;

#[cfg(test)]
mod test {
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
    fn test_html_write() {
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

        println!("{:?}", TerminalCapabilities::detect().name);
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

        println!("{}", w.to_string());
    }
}
