//! Writing serialized events out.
//!
//! This module contains the [`WriteEvent`] trait which is what you must implement
//! to write serialized events out to a file, socket, terminal or other `Writer`.
use crate::SerdeFormat;
use serde::Serialize;
use std::io::{self, Stderr, Stdout, Write};
use std::sync::{Arc, Mutex};

// FIXME: don't panic when getting a poisoned Mutex

mod nonblocking;

pub use nonblocking::{FlushGuard, NonBlocking, NonBlockingBuilder};

/// Serializes the tracing event by constructing a [Writer](std::io::Write)
/// and calling [`SerdeFormat::serialize`] on `fmt` with the Writer and `event`.
///
/// Note that this takes a `&self`, not a `&mut self`, as [`WriteEvent::write`] may
/// be called concurrently from multiple threads.  This means implementors need to implement
/// some kind of synchronisation mechanisism (such as a [`Mutex`](std::sync::Mutex)) to produce a mutable
/// `Write` instance that can be passed to
///
/// It is automatically implemented for `Arc<Mutex<W>>` where `W: Write` so you can give a
/// `Arc::new(Mutex::new(writer))` to [`SerdeLayerBuilder::with_writer`](crate::subscriber::SerdeLayerBuilder::with_writer).
pub trait WriteEvent {
    /// On encountering an IO error, print a warning.
    ///
    /// Default is to ignore IO errors silently.
    fn warn_on_error(self) -> WarnOnError<Self>
    where
        Self: Sized,
    {
        WarnOnError::new(self)
    }

    /// On encountering an IO error, panic.
    ///
    /// Default is to ignore IO errors silently.
    fn panic_on_error(self) -> PanicOnError<Self>
    where
        Self: Sized,
    {
        PanicOnError::new(self)
    }

    /// Serializes the tracing event using the supplied `fmt`.
    fn write(&self, fmt: impl SerdeFormat, event: impl Serialize) -> io::Result<()>;
}

impl<'a, T: WriteEvent> WriteEvent for &'a T {
    fn write(&self, fmt: impl SerdeFormat, event: impl Serialize) -> io::Result<()> {
        <T as WriteEvent>::write(self, fmt, event)
    }
}

impl<T: WriteEvent> WriteEvent for Arc<T> {
    fn write(&self, fmt: impl SerdeFormat, event: impl Serialize) -> io::Result<()> {
        T::write(&*self, fmt, event)
    }
}

macro_rules! impl_writeevent_for_stdpipe {
    ($t:path) => {
        impl WriteEvent for $t {
            fn write(&self, fmt: impl SerdeFormat, event: impl Serialize) -> io::Result<()> {
                fmt.serialize(self.lock(), event)
            }
        }
    };
}

impl_writeevent_for_stdpipe!(Stdout);
impl_writeevent_for_stdpipe!(Stderr);

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
    pub fn new(inner: T) -> Self {
        PanicOnError(inner)
    }
}

impl<T: WriteEvent> WriteEvent for PanicOnError<T> {
    fn write(&self, fmt: impl SerdeFormat, event: impl Serialize) -> io::Result<()> {
        if let Err(e) = self.0.write(fmt, event) {
            panic!("{}", fail_message!(e))
        }
        Ok(())
    }
}

/// A wrapper type for printing a warning when the inner `WriteEvent`
/// returns an error.
///
/// The default behaviour of [`SerdeLayer`](crate::SerdeLayer) is to silently ignore any
/// errors returned by the [`WriteEvent`] writer.
pub struct WarnOnError<T>(pub T);

impl<T: WriteEvent> WarnOnError<T> {
    /// Wrapper the inner `WriteEvent`, printing the error to `STDERR` whenever
    /// [`write`](WriteEvent::write) method returns an error.
    pub fn new(inner: T) -> Self {
        WarnOnError(inner)
    }
}

impl<T: WriteEvent> WriteEvent for WarnOnError<T> {
    fn write(&self, fmt: impl SerdeFormat, event: impl Serialize) -> io::Result<()> {
        if let Err(e) = self.0.write(fmt, event) {
            eprintln!("{}", fail_message!(e))
        }
        Ok(())
    }
}
