//! This module provides the [`StreamFormat`] trait which describes formats that can be stream-deserialized
//! into [`Events`](crate::Event) from a [Reader](std::io::Read).  
//!
//! It also provides some pretty printing of [`Event`]s.
use crate::Event;
use std::fs::File;
use std::io::{self, BufReader};
use std::path::Path;

mod pprint;
pub use pprint::{FmtEvent, PrettyPrinter};

/// Describes how events should be deserialized for a serde-supported format.
///
/// # Implementing
/// `StreamFormat::Stream` is typically a wrapper around a serde data format crate's [`Deserializer`](serde::Deserializer)
/// implementation, which implements `Iterator`.  Any deserialization errors that aren't I/O errors should be panicked.
///
/// [`iter_reader`](StreamFormat::iter_reader) should simply construct this wrapper type.
///
/// You implement this as `impl<R: std::io::Read> StreamFormat<R> for MyFormat { ... }`
pub trait StreamFormat<R>: Sized {
    /// The type of the stream.
    type Stream: Iterator<Item = io::Result<Event>>;

    /// Construct the stream from the supplied reader.
    ///
    /// The stream implements `Iterator<Item=io::Result<Event>>`, so it can be used like so:
    ///```no_run
    /// use tracing_subscriber_serde::{Event, consumer::StreamFormat, format::Json};
    ///
    /// fn serializes_events_in_json(buf: &mut Vec<u8>) {
    ///     todo!()
    /// }
    ///
    /// let mut buffer = Vec::new();
    ///
    /// serializes_events_in_json(&mut buffer);
    ///  
    /// for r in Json.iter_reader(&*buffer) {
    ///     let event: Event = r.unwrap();
    /// }
    ///
    ///```
    fn iter_reader(&self, reader: R) -> Self::Stream;
}

/// A convience trait for constructing a stream and iterating over it in one step.
/// It is automatically implemented if [`StreamFormat`] is implemented.
pub trait IterFile: StreamFormat<BufReader<File>> {
    /// Open the file and parse events using this format.
    ///
    /// If opening the file fails, the iterator will return one item, which is the
    /// IO error.
    fn iter_file(&self, path: impl AsRef<Path>) -> TryOpenStream<Self::Stream> {
        let file = match File::open(path) {
            Ok(f) => f,
            Err(e) => return TryOpenStream::err_on_open(e),
        };

        TryOpenStream::success(self.iter_reader(BufReader::new(file)))
    }
}

impl<T: StreamFormat<BufReader<File>>> IterFile for T {}

pub enum TryOpenStream<I> {
    OpenError(Option<io::Error>),
    Success(I),
}

impl<I> TryOpenStream<I> {
    pub fn err_on_open(e: io::Error) -> Self {
        TryOpenStream::OpenError(Some(e))
    }

    pub fn success(i: I) -> Self {
        TryOpenStream::Success(i)
    }
}

impl<T, I: Iterator<Item = io::Result<T>>> Iterator for TryOpenStream<I> {
    type Item = io::Result<T>;

    fn next(&mut self) -> Option<Self::Item> {
        use TryOpenStream::*;
        match self {
            OpenError(err) => err.take().map(io::Result::Err),
            Success(iter) => iter.next(),
        }
    }
}
