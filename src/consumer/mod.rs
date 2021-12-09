use crate::Event;
use std::path::Path;
use std::io::{Result as IoResult, Read, BufReader};

mod pprint;
pub use pprint::{PrettyPrinter, FmtEvent};


/// A convenience function for iteration over [`Json`](crate::format::Json)-serialized events
/// in a [`Reader`](std::io).
pub fn iter_json_reader(reader: impl Read) -> impl Iterator<Item=IoResult<Event>> {
  serde_json::Deserializer::from_reader(reader)
    .into_iter::<Event>()
    .map(|r| r.map_err(From::from))
}

/// A convenience function for iteration over [`Json`](crate::format::Json)-serialized events
/// in a file.
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