use flume::{Receiver, Sender, TrySendError};
use std::io::{Write, self};

use std::thread::JoinHandle;

use serde::Serialize;

use crate::SerdeFormat;
use super::WriteEvent;

pub const DEFAULT_BUFFERED_RECORDS_LIMIT: usize = 128_000;

/// Constructs a [`NonBlocking`].
#[derive(Clone, Debug)]
pub struct NonBlockingBuilder {
  lossy: bool,
  max_buffered_records: usize,
}

impl Default for NonBlockingBuilder {
  fn default() -> Self {
    NonBlockingBuilder {
      lossy: false,
      max_buffered_records: DEFAULT_BUFFERED_RECORDS_LIMIT,
    }
  }
}

const PANIC_MSG_DEAD_WRITER : &'static str = "writer thread has died";


impl NonBlockingBuilder {
  /// Sets the maximum number of events buffered. See [`NonBlockingBuilder::lossy`] on behaviour
  /// when the buffer is full.
  pub fn buf_size(mut self, sz: usize) -> Self {
    self.max_buffered_records = sz;
    self
  }

  /// If the buffer is full, events will be dropped if `lossy = true`,
  /// otherwise the `NonBlocking` will block until the buffer has space.
  pub fn lossy(mut self, lossy: bool) -> Self {
    self.lossy = lossy;
    self
  }

  /// Finish configuration.
  pub fn finish<W: Write + Send + 'static>(self, writer: W) -> (NonBlocking, FlushGuard) {
    let guard = WriterThread::spawn(writer, self.max_buffered_records);

    let writer = NonBlocking {
      sender: guard.sender.clone(),
      lossy: self.lossy,
      message_buf_initial_capacity: self.max_buffered_records,
    };
    (writer, guard)
  }
}


#[derive(Clone, Debug, Eq, PartialEq)]
enum Message {
  Record(Vec<u8>),
  Shutdown,
}

/// A "non-blocking" writer which spawns a dedicated I/O thread and feeds
/// it serialized events using message passing.
///
/// Non-blocking is in quotes because it is only non-blocking if `lossy` is set to `false`
/// with [`NonBlockingBuilder::lossy`].
#[derive(Clone, Debug)]
pub struct NonBlocking {
  sender: Sender<Message>,
  lossy: bool,
  message_buf_initial_capacity: usize,
}

/// The writer thread of [`NonBlocking`] will shutdown when this RAII guard is dropped,
/// flushing any buffered events.
#[derive(Debug)]
pub struct FlushGuard {
  handle: Option<JoinHandle<()>>,
  sender: Sender<Message>,
}

impl Drop for FlushGuard {
  fn drop(&mut self) {
    self.sender.send(Message::Shutdown).expect(PANIC_MSG_DEAD_WRITER);
    self.handle.take().unwrap().join().unwrap();
  }
}

impl NonBlocking {
  pub fn new() -> NonBlockingBuilder { NonBlockingBuilder::default() }
}

impl WriteEvent for NonBlocking {
  fn write(&self, fmt: impl SerdeFormat, event: impl Serialize) -> io::Result<()> {
    let mut buf = Vec::with_capacity(fmt.message_size_hint());
    fmt.serialize(&mut buf, event).expect("bug: Failed to serialize event");
    if self.lossy {
      match self.sender.try_send(Message::Record(buf)) {
        Err(TrySendError::Disconnected(_)) => panic!("{}", PANIC_MSG_DEAD_WRITER),
        _ => {},
      }
    } else {
      self.sender.send(Message::Record(buf)).expect(PANIC_MSG_DEAD_WRITER);
    }
    Ok(())
  }
}

struct WriterThread<W> {
  queue: Receiver<Message>,
  writer: W,
}

impl<W: Write + Send + 'static> WriterThread<W> {
  pub fn spawn(writer: W, max_buffered: usize) -> FlushGuard {
    let (sender, receiver) = flume::bounded(max_buffered);

    let mut thread = WriterThread {
      queue: receiver,
      writer,
    };

    let thread_handle = std::thread::spawn(move || thread.run());

    FlushGuard {
      handle: Some(thread_handle),
      sender,
    }
  }

  fn handle_io_err(&mut self, err: Option<io::Error>) {
    if let Some(e) = err {
      // TODO allow user to shut this up
      eprintln!("WriterThread: failed to write log record: {}", e)
    }
  }

  fn handle_message(&mut self, msg: Message) {
    match msg {
      Message::Record(data) => {
        let e = self.writer.write(&data).err();
        self.handle_io_err(e);
      }
      Message::Shutdown => unreachable!(),
    }
  }

  fn drain(&mut self) {
    while let Ok(msg) = self.queue.try_recv() {
      // We only ever create one Message::Shutdown, which is sent when the
      // guard is dropped, so this will
      self.handle_message(msg);
    }
  }

  fn run(&mut self) {
    loop {
      match self.queue.recv().unwrap() {
        Message::Shutdown => {
          self.drain();
          break;
        },
        msg => self.handle_message(msg),
      }
    }

    // Senders have hung up
    let e = self.writer.flush().err();
    self.handle_io_err(e);
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::time::Duration;
  use std::sync::{Mutex, Arc};

  use crate::format::Json;

  type Buffer = Arc<Mutex<Vec<u8>>>;

  struct TestWriter {
    buffer: Buffer,
    write_size: Option<usize>,
    interrupts: Option<usize>,
    write_counter: usize,
    wait: Option<Receiver<()>>
  }

  struct Signal(Sender<()>);

  impl Signal {
    pub fn send(&self) {
      self.0.send_timeout((), Duration::from_secs(5))
        .expect("writer stalled")
    }
  }

  impl Write for TestWriter {
    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
      if let Some(signal) = self.wait.as_ref() {
        eprintln!("TestWriter: waiting for signal");
        signal.recv_timeout(Duration::from_secs(5)).expect("writer stalled");
        eprintln!("TestWriter: continuing");
      }

      self.write_counter += 1;
      if let Some(n) = self.interrupts {
        if self.write_counter % n == 0 {
          eprintln!("TestWriter: interrupted");
          return Err(io::ErrorKind::Interrupted.into());
        }
      }
      let n = self.write_size.unwrap_or(buf.len()).min(buf.len());
      self.buffer.lock().unwrap().extend_from_slice(&buf[..n]);
      Ok(n)
    }

    fn flush(&mut self) -> Result<(), io::Error> {
      eprintln!("TestWriter: flushed");
      Ok(())
    }
  }

  impl TestWriter {
    pub fn new(interrupts: Option<usize>, write_size: Option<usize>) -> Self {
      let buffer = Arc::new(Mutex::new(Vec::new()));
      TestWriter {
        buffer,
        write_counter: 0,
        interrupts,
        wait: None,
        write_size,
      }
    }
    pub fn signalled(&mut self) -> Signal {
      assert!(self.wait.is_none());
      let (s, r) = flume::bounded(0);
      self.wait = Some(r);
      Signal(s)
    }
  }

  #[test]
  fn interrupts() {
    let writer = TestWriter::new(None, None);
    let buffer = Arc::clone(&writer.buffer);
    let (writer, g) = NonBlocking::new().finish(writer);

    for message in 0..5 {
      // First two messages will get buffered, others will be dropped.
      writer.write(Json, message).unwrap();
    }

    drop(g);
    let output = String::from_utf8(buffer.lock().unwrap().clone()).unwrap();
    assert_eq!(output, "0\n1\n2\n3\n4\n");
  }

  #[test]
  fn drops_logs_when_full() {
    let mut writer = TestWriter::new(None, None);
    let writer_continue = writer.signalled();
    let buffer = Arc::clone(&writer.buffer);

    let num_buffered = 2;

    let (writer, g) = NonBlocking::new()
      .lossy(true)
      .buf_size(num_buffered)
      .finish(writer);

    for message in 0..10 {
      // First two messages will get buffered, others will be dropped.
      writer.write(Json, message).unwrap();
    }

    for _ in 0..num_buffered {
      writer_continue.send();
    }

    writer.write(Json, "hello world").unwrap();
    writer_continue.send();

    drop(g);

    let output = String::from_utf8(buffer.lock().unwrap().clone()).unwrap();
    assert_eq!(output, "0\n1\n\"hello world\"\n");
  }
}
