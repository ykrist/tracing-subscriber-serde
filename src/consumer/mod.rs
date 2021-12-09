//! This module provides some pretty printing of [`Event`]s and convenience functions for
//! deserializing events that were stored in [`Json`](crate::format::Json) format.
//!
//! The source of module serves as an example of how to consume the serialized events.
use crate::Event;
use std::path::Path;
use std::io::{Result as IoResult, Read, BufReader};

mod pprint;
pub use pprint::{PrettyPrinter, FmtEvent};


/// Iterate [`Json`](crate::format::Json)-serialized events from a [`Reader`](std::io).
pub fn iter_json_reader(reader: impl Read) -> impl Iterator<Item=IoResult<Event>> {
  serde_json::Deserializer::from_reader(reader)
    .into_iter::<Event>()
    .map(|r| r.map_err(From::from))
}

/// Iterate [`Json`](crate::format::Json)-serialized events from a file.
pub fn iter_json_file(p: impl AsRef<Path>) -> impl Iterator<Item=IoResult<Event>> {
  let mut open_error = None;
  let mut file = None;

  match std::fs::File::open(p) {
    Ok(f) => file = Some(BufReader::new(f)),
    Err(e) => open_error = Some(IoResult::Err(e)),
  }

  let records  = file.into_iter().flat_map(iter_json_reader);
  open_error.into_iter().chain(records)
}

#[cfg(test)]
mod tests {

}