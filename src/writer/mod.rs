use serde::Serialize;
use std::io::{Stdout, Stderr, Write, self};
use std::sync::Mutex;
use crate::subscriber::SerdeFormat;

mod nonblocking;

pub use nonblocking::{FlushGuard, NonBlocking, Builder};

/// Serializes the tracing event by constructing a [Writer](std::io::Write)
/// and calling [`SerdeFormat::serialize`] on `fmt` with the Writer and `event.
///
/// Note that this takes a `&self`, not a `&mut self`, as [`WriteEvent::write`] may
/// be called concurrently from multiple threads.  This means implementors need to implement
/// some kind of synchronisation mechanisism (such as a [`Mutex`](std::sync::Mutex)) to produce a mutable
/// `Write` instance that can be passed to
///
/// It is automatically implemented for `Mutex<W>` where `W: Write` so you can give a
/// `Mutex::new(writer)` to [`SerdeLayerBuilder::with_writer`](crate::subscriber::SerdeLayerBuilder::with_writer).
pub trait WriteEvent {
  /// Serializes the tracing event using the supplied`fmt`.
  fn write(&self, fmt: impl SerdeFormat, event: impl Serialize) -> io::Result<()>;
}

impl<'a, T: WriteEvent> WriteEvent for &'a T {
  fn write(&self, fmt: impl SerdeFormat, event: impl Serialize) -> io::Result<()> {
    <T as WriteEvent>::write(self, fmt, event)
  }
}


macro_rules! impl_writerecord_for_stdpipe {
  ($t:path) => {
    impl WriteEvent for $t {
      fn write(&self, fmt: impl SerdeFormat, event: impl Serialize) -> io::Result<()> {
        fmt.serialize(self.lock(), event)
      }
    }
  };
}

impl_writerecord_for_stdpipe!(Stdout);
impl_writerecord_for_stdpipe!(Stderr);

impl<W: Write> WriteEvent for Mutex<W> {
  fn write(&self, fmt: impl SerdeFormat, event: impl Serialize) -> io::Result<()> {
    let writer = &mut *self.lock().expect("Writer mutex poisoned");
    fmt.serialize(writer, event)
  }
}

macro_rules! fail_message {
    ($error:ident) => {
        format_args!("tracing: failed to write to log: {}", $error)
    };
}

/// A wrapper type for panicking when the inner `WriteEvent`
/// returns an error.
///
/// The default behaviour of [`SerdeLayer`](crate::SerdeLayer) is to silently ignore any
/// errors returned by the [`WriteEvent`] writer.
pub struct PanicOnError<T>(T);

impl<T: WriteEvent> PanicOnError<T> {
  /// Wrapper the inner `WriteEvent`, panicking whenever its
  /// [`write`](WriteEvent::write) method returns an error.
  pub fn new(inner: T) -> Self { PanicOnError(inner) }
}

impl<T: WriteEvent> WriteEvent for PanicOnError<T> {
  fn write(&self, fmt: impl SerdeFormat, event: impl Serialize) -> io::Result<()> {
    if let Err(e) = self.0.write(fmt, event) {
      panic!("{}", fail_message!(e))
    }
    Ok(())
  }
}

/// A wrapper type for panicking when the inner `WriteEvent`
/// returns an error.
///
/// The default behaviour of [`SerdeLayer`](crate::SerdeLayer) is to silently ignore any
/// errors returned by the [`WriteEvent`] writer.
pub struct WarnOnError<T>(pub T);

impl<T: WriteEvent> WarnOnError<T> {
  /// Wrapper the inner `WriteEvent`, printing the error to `STDERR` whenever
  /// [`write`](WriteEvent::write) method returns an error.
  pub fn new(inner: T) -> Self { WarnOnError(inner) }
}

impl<T: WriteEvent> WriteEvent for WarnOnError<T> {
  fn write(&self, fmt: impl SerdeFormat, event: impl Serialize) -> io::Result<()> {
    if let Err(e) = self.0.write(fmt, event) {
      eprintln!("{}", fail_message!(e))
    }
    Ok(())
  }
}