use std::io::{self, Write, Error, ErrorKind};
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

fn test_html_write() {
    
}

pub(crate) fn write_markdown_string(input: &str) -> Result<String, anyhow::Error> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    let parser = Parser::new_ext(&input, options);
    let syntax_set = SyntaxSet::load_defaults_nonewlines();

    let mut w = Writter::default();
    mdcat::push_tty(
        &mut w,
        &TerminalCapabilities::detect(),
        TerminalSize::detect().ok_or(anyhow::Error::new(Error::new(ErrorKind::Other, "could not detect terminal")))?,
        parser,
        &std::path::Path::new("/"),
        ResourceAccess::RemoteAllowed,
        syntax_set,
    ).map_err(|e| anyhow::Error::new(Error::new(ErrorKind::Other, e.to_string())))?;

    Ok(w.to_string())
}
