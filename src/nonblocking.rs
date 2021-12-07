

use rtrb::{Producer, Consumer, RingBuffer};
use std::sync::mpsc::{Receiver, Sender, SyncSender};
use std::io::{Write, self};

pub use tracing_subscriber::fmt::MakeWriter;
use std::cell::RefCell;
use std::thread::JoinHandle;

// Try not to write things smaller than 4 KB
const DEFAULT_MIN_WRITE_SIZE: usize = 0x1000;
const DEFAULT_BUF_SIZE: usize = 16 * DEFAULT_MIN_WRITE_SIZE;

#[derive(Clone, Debug)]
pub struct Builder {
  lossy: bool,
  buf_size: usize,
  min_write_size: usize,
}

impl Default for Builder {
  fn default() -> Self {
    Builder {
      lossy: false,
      buf_size: DEFAULT_BUF_SIZE,
      min_write_size: DEFAULT_MIN_WRITE_SIZE,
    }
  }
}

impl Builder {
  pub fn min_write_size(mut self, sz: usize) -> Self {
    self.min_write_size = sz;
    self
  }

  pub fn buf_size(mut self, sz: usize) -> Self {
    self.buf_size = sz;
    self
  }

  pub fn finish<W: Write + Send + 'static>(self, writer: W) -> WriterHandle {
    let (prod, cons) = RingBuffer::new(self.buf_size);
    assert!(self.buf_size >= self.min_write_size);

    let handle = WriterThread::spawn(writer, cons, self.min_write_size);
    WriterHandle::new(prod, handle, self.lossy)
  }
}

pub fn nonblocking() -> Builder { Builder::default() }

pub use writer_handle::WriterHandle;
mod writer_handle {
  use super::*;
  use std::mem::MaybeUninit;
  use rtrb::chunks::ChunkError;

  struct MutState {
    buf: Producer<u8>,
    num_dropped: u32,
  }

  pub struct WriterHandle {
    inner: RefCell<MutState>,
    handle: Option<WriterThreadHandle>,
    lossy: bool,
  }

  fn write_slice_exact(thread: &WriterThreadHandle, ringbuf: &mut Producer<u8>, buf: &[u8]) -> Result<(), ChunkError> {
    debug_assert!(!buf.is_empty());
    let mut c = ringbuf.write_chunk_uninit(buf.len())?;
    let (left, right) = c.as_mut_slices();
    let n = left.len();
    MaybeUninit::write_slice(left, &buf[..n]);
    MaybeUninit::write_slice(right, &buf[n..]);
    // SAFETY:
    // MaybeUninit::write_slice will panic if the slice lengths are unequal, so
    // left.len() + right.len() == buf.len()
    eprintln!("write buf {}", buf.len());
    unsafe { c.commit_all() };
    thread.notify();
    Ok(())
  }

  impl WriterHandle {
    pub(super) fn new(buf: Producer<u8>, handle: WriterThreadHandle, lossy: bool) -> Self {
      WriterHandle {
        inner: RefCell::new(MutState {
          buf,
          num_dropped: 0,
        }),
        handle: Some(handle),
        lossy: lossy,
      }
    }

    pub fn write(&self, mut buf: &[u8]) {
      let MutState { buf: ref mut ringbuf, ref mut num_dropped } = &mut *self.inner.borrow_mut();
      let writer_thread = self.handle.as_ref().unwrap();

      let mut avail = match write_slice_exact(writer_thread, ringbuf, buf) {
        Ok(()) => return,
        Err(ChunkError::TooFewSlots(_)) if self.lossy => return, // Non-blocking behaviour
        Err(ChunkError::TooFewSlots(n)) => n,
      };

      let mut buf = buf;

      while !buf.is_empty() {
        if avail == 0 {
          writer_thread.wait_for_space();
          avail = ringbuf.slots();
          debug_assert!(avail > 0);
        }

        avail = avail.min(buf.len());
        write_slice_exact(writer_thread, ringbuf, &buf[..avail]).unwrap();
        buf = &buf[avail..];
        avail = ringbuf.slots();
        // eprintln!("{:?}", buf)
      }
    }
  }

  impl Drop for WriterHandle {
    fn drop(&mut self) {
      self.handle.take().expect("bug: double drop").shutdown();
    }
  }
}


pub struct WriterHandleRef<'a>(&'a WriterHandle);

impl Write for WriterHandleRef<'_> {
  fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {
    self.0.write(buf);
    Ok(buf.len())
  }

  fn flush(&mut self) -> Result<(), io::Error> { Ok(()) }
}

impl<'a> MakeWriter<'a> for WriterHandle {
  type Writer = WriterHandleRef<'a>;

  fn make_writer(&'a self) -> Self::Writer { WriterHandleRef(self) }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum Message {
  WakeUp,
  MakeSpace,
}

struct WriterThreadHandle {
  thread_handle: JoinHandle<()>,
  data_available: Sender<Message>,
  space_available: Receiver<()>,
}

const PANIC_MSG_DEAD_WRITER : &'static str = "writer thread has died";

impl WriterThreadHandle {
  fn notify(&self) {
    self.data_available.send(Message::WakeUp).expect(PANIC_MSG_DEAD_WRITER);
  }

  fn wait_for_space(&self) {
    eprintln!("blocking for space...");
    self.data_available.send(Message::MakeSpace).expect(PANIC_MSG_DEAD_WRITER);
    self.space_available.recv().expect(PANIC_MSG_DEAD_WRITER);
    eprintln!("space!");
  }

  fn shutdown(self) {
    let Self { thread_handle, space_available, data_available} = self;
    // Tell the writer to shutdown
    drop(space_available);
    drop(data_available);
    // Wait for shutdown
    thread_handle.join().unwrap();
  }
}

struct WriterThread<W> {
  min_write_size: usize,
  data_available: Receiver<Message>,
  space_available: SyncSender<()>,
  buf: Consumer<u8>,
  writer: W,
}

impl<W: Write + Send + 'static> WriterThread<W> {
  pub fn spawn(writer: W, buf: Consumer<u8>, min_write_size: usize) -> WriterThreadHandle {
    eprintln!("spawn");
    let (data_available_send, data_available_recv) = std::sync::mpsc::channel();
    let (space_available_send, space_available_recv) = std::sync::mpsc::sync_channel(0);
    eprintln!("Capacity = {}", buf.buffer().capacity());

    let mut thread = WriterThread {
      min_write_size,
      buf,
      writer,
      data_available: data_available_recv,
      space_available: space_available_send,
    };

    let thread_handle = std::thread::spawn(move || thread.run());

    WriterThreadHandle {
      thread_handle,
      space_available: space_available_recv,
      data_available: data_available_send
    }
  }

  fn handle_io_err<T>(err: Result<T, io::Error>) -> Option<T> {
    match err {
      Err(_err) => {
        // TODO: maybe log to stderr?
        None
      },
      Ok(x) => Some(x)
    }
  }

  fn flush_buffer(&mut self) {
    let n = self.buf.slots();
    let chunks = self.buf.read_chunk(n).unwrap();
    let (left, right) = chunks.as_slices();
    eprintln!("l={:?} r={:?}", left, right);
    // Correctly handles ErrorKind::Interrupted
    Self::handle_io_err(self.writer.write_all(left));
    Self::handle_io_err(self.writer.write_all(right));
    chunks.commit_all();
  }

  fn run(&mut self) {
    while let Ok(msg) = self.data_available.recv() {
      dbg!(msg);
      if self.min_write_size <= self.buf.slots() {
        self.flush_buffer();
      }

      if msg == Message::MakeSpace {
        // We received this message because the buffer is full.
        // Since self.min_write_size <= self.buf.buffer().capacity(),
        // this means we must have called self.flush_buffer() in the previous iteration of
        // the while loop and we simply need to notify the other end that there is
        // space in the buffer again.
        debug_assert!(self.buf.slots() < self.buf.buffer().capacity());
        self.space_available.send(());
      }
    }

    // Sender has hung up, indicating we should flush and close the file.
    self.flush_buffer();
    self.writer.flush();
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::sync::{Mutex, Arc};
  use std::cmp::min;

  type Buffer = Arc<Mutex<Vec<u8>>>;

  struct TestWriter {
    buffer: Buffer,
    write_size: Option<usize>,
    interrupts: Option<usize>,
    write_counter: usize,
  }


  impl Write for TestWriter {
    fn write(&mut self, buf: &[u8]) -> Result<usize, io::Error> {

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
        write_size,
      }
    }
  }

  fn payload(size: usize) -> Vec<u8> {
    (0..size).map(|x| x as u8).collect()
  }

  fn check_same(buf: &Buffer, data: &[u8]) {
    let buf = buf.lock().unwrap();
    assert_eq!(buf.as_slice(), data)
  }

  fn test_buf_size(payload_size: usize, write_size: Option<usize>, interrupts: Option<usize>, buf_size: usize, min_write_size: usize) {
    let writer = TestWriter::new(interrupts, write_size);
    let buffer = Arc::clone(&writer.buffer);
    let handle = nonblocking()
      .buf_size(buf_size)
      .min_write_size(min_write_size)
      .finish(writer);
    let data = payload(payload_size);
    handle.write(&data);
    drop(handle);
    check_same(&buffer, &data);
  }

  fn test(payload_size: usize, write_size: Option<usize>, interrupts: Option<usize>) {
    test_buf_size(payload_size, write_size, interrupts, DEFAULT_BUF_SIZE, DEFAULT_MIN_WRITE_SIZE);
  }

  #[test]
  fn simple() {
    test(10, None, None);
  }


  #[test]
  fn multiple_writes() {
    test(100, Some(40), None);
  }

  #[test]
  fn interrupts() {
    test(100, Some(9), Some(3));
  }

  #[test]
  fn pathological_buf_size() {
    test_buf_size(10, None, None, 13, 3);
  }

  #[test]
  fn ringbuf_wrap_around() {
    let writer = TestWriter::new(None, None);
    let buffer = Arc::clone(&writer.buffer);
    let handle = nonblocking()
      .buf_size(12)
      .min_write_size(12)
      .finish(writer);

    let data = payload(100);

    for chunk in data.chunks(10) {
      handle.write(&chunk);
    }

    drop(handle);
    check_same(&buffer, &data);
  }

}
