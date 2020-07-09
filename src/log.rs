use std::{
    fs, io,
    path::{Path, PathBuf},
};

use tracing_appender::{
    non_blocking,
    non_blocking::{NonBlocking, WorkerGuard},
};

#[derive(Clone, Debug)]
pub struct LogWriter {
    path: PathBuf,
}

impl LogWriter {
    pub fn spawn_logger<P: AsRef<Path>>(path: P) -> (NonBlocking, WorkerGuard) {
        let log = LogWriter {
            path: path.as_ref().to_path_buf(),
        };
        non_blocking(log)
    }
}

impl io::Write for LogWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut file = fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(&self.path)?;

        file.write_all(buf).map(|_| buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
