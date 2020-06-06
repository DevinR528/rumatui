use std::{
    fmt::{self, Display},
    io::{self, ErrorKind, Write},
};

use comrak;
use mdcat::{self, ResourceAccess, Settings, TerminalCapabilities, TerminalSize};
use pulldown_cmark::{Options, Parser};
use syntect::parsing::SyntaxSet;

use crate::error::{Error, Result};

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

pub(crate) fn markdown_to_terminal(input: &str) -> Result<String> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    let parser = Parser::new_ext(&input, options);
    let syntax_set = SyntaxSet::load_defaults_nonewlines();

    let settings = Settings {
        terminal_capabilities: TerminalCapabilities::detect(),
        terminal_size: TerminalSize::detect().ok_or(Error::from(io::Error::new(
            ErrorKind::Other,
            "could not detect terminal",
        )))?,
        resource_access: ResourceAccess::LocalOnly,
        syntax_set,
    };
    let mut w = Writer::default();
    mdcat::push_tty(&settings, &mut w, &std::path::Path::new("/"), parser)
        .map_err(|e| Error::from(io::Error::new(ErrorKind::Other, e.to_string())))?;

    Ok(w.to_string())
}

pub(crate) fn markdown_to_html(input: &str) -> String {
    comrak::markdown_to_html(input, &comrak::ComrakOptions::default())
}
