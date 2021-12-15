//! This module provides some pretty printing of [`Event`]s and convenience functions for
//! deserializing events that were stored in [`Json`](crate::format::Json) format.
//!
//! The source of module serves as an example of how to consume the serialized events.
#![allow(missing_docs)] // FIXME: remove

use crate::Event;
use std::io::{BufReader, self};
use std::path::Path;
use std::fs::File;

mod pprint;
pub use pprint::{FmtEvent, PrettyPrinter};

pub trait StreamFormat<R>: Sized {
    type Stream: Iterator<Item=io::Result<Event>>;
    
    fn iter_reader(self, reader: R) -> Self::Stream;
}

pub trait IterFile: StreamFormat<BufReader<File>> {
    fn iter_file(self, path: impl AsRef<Path>) -> TryOpenStream<Self::Stream> {
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
    pub fn err_on_open(e: io::Error) -> Self { TryOpenStream::OpenError(Some(e)) }

    pub fn success(i: I) -> Self { TryOpenStream::Success(i) }
}

impl<T, I: Iterator<Item=io::Result<T>>> Iterator for TryOpenStream<I> {
    type Item = io::Result<T>;

    fn next(&mut self) -> Option<Self::Item> {
        use TryOpenStream::*;
        match self {
            OpenError(err) => err.take().map(io::Result::Err),
            Success(iter) => iter.next(),
        }
    }
}
