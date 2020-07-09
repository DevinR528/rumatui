use std::{
    fs,
    io::{self, Write},
    path::Path,
};

use tokio::{runtime::Handle, sync::mpsc, task::JoinHandle};

// TODO make the file and writer async
//

#[derive(Clone, Debug)]
pub struct LogWriter(mpsc::UnboundedSender<Vec<u8>>);

impl io::Write for LogWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0
            .send(buf.to_vec())
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "LogWriter send failed"))?;
        Ok(0)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub struct Logger {
    snd: LogWriter,
}

impl Logger {
    pub fn spawn_logger<P: AsRef<Path>>(
        path: P,
        exec: Handle,
    ) -> io::Result<(Self, JoinHandle<()>)> {
        let (snd, mut rcv) = mpsc::unbounded_channel();
        let file = fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(path)?;

        let mut file = io::BufWriter::new(file);

        Ok((
            Self {
                snd: LogWriter(snd),
            },
            exec.spawn(async move {
                loop {
                    if let Some(msg) = rcv.recv().await {
                        if let Err(err) = file.write_all(&msg) {
                            panic!("logger panicked receiving log event: {}", err)
                        }
                    }
                }
            }),
        ))
    }
}

impl tracing_subscriber::fmt::MakeWriter for Logger {
    type Writer = LogWriter;

    fn make_writer(&self) -> Self::Writer {
        self.snd.clone()
    }
}
