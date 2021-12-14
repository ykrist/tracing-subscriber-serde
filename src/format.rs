//! Serde formats and supporting trait.
use std::io::Write;
use serde::Serialize;

/// The main adaptor trait for logging tracing events with a [serde-supported format](https://docs.rs/serde).
///
/// Implementing [`SerdeFormat::serialize`] typically involves constructing a [`serde::Serializer`] from the `buf` writer
/// and calling `event.serialize(serializer)`.
///
/// The implementation of
///
/// # Examples
/// Below is the implementation for [`Json`].
/// ```ignore
/// impl SerdeFormat for Json {
///   fn message_size_hint(&self) -> usize { 512 }
///
///   fn serialize(&self, mut buf: impl Write, event: impl Serialize) -> std::io::Result<()> {
///     serde_json::to_writer(&mut buf, &event)?;
///     buf.write("\n".as_bytes())?;
///     Ok(())
///   }
/// }
///
/// ```
pub trait SerdeFormat {
  /// Provide a hint at the expect size of the serialized message.
  /// Implementors of [`WriteRecord`](crate::WriteEvent) can use this to initialise a buffer capacity.
  fn message_size_hint(&self) -> usize;

  /// Perform the serialization.
  fn serialize(&self, buf: impl Write, event: impl Serialize) -> std::io::Result<()>;
}

impl<'a, T: SerdeFormat> SerdeFormat for &'a T {
  fn message_size_hint(&self) -> usize { T::message_size_hint(self) }

  fn serialize(&self, buf: impl Write, event: impl Serialize) -> std::io::Result<()> {
    T::serialize(self, buf, event)
  }
}


#[derive(Copy, Clone, Debug)]
/// Serialize each event using a compact JSON format, separated by newlines.
pub struct Json;

impl SerdeFormat for Json {
  fn message_size_hint(&self) -> usize { 512 }

  fn serialize(&self, mut buf: impl Write, event: impl Serialize) -> std::io::Result<()> {
    serde_json::to_writer(&mut buf, &event)?;
    buf.write("\n".as_bytes())?;
    Ok(())
  }
}




#[cfg(test)]
mod tests {
  use super::*;
  use std::{num::NonZeroU64, time::Duration};
  use std::collections::HashMap;
  use crate::test_utils::eq_event;
use crate::{Event, EventKind, time::{SpanTime, UnixTime}, Level, Span, FieldValue};
  use itertools::{iproduct, Itertools};


  fn test_format<F, D, I>(fmt: F, evnts: I, deserialize: D) 
    where 
      F: SerdeFormat,
      D: Fn(&[u8]) -> Vec<Event>,
      I: Iterator<Item=Event>,
  {
    let mut events = Vec::new();
    let mut buffer = Vec::new();

    for e in evnts {
      fmt.serialize(&mut buffer, &e).unwrap();
      events.push(e);
    }

    let deserialized = deserialize(&buffer);
    dbg!(events.len());
    assert_eq!(events.len(), deserialized.len());

    for (orig, de) in events.iter().zip(&deserialized) {
        if !eq_event(orig, de) {
            eprintln!("  serialized = {:?}", orig);
            eprintln!("deserialized = {:?}", de);
            panic!("deserialization doesn't match serialization")
        }
    }
  }

  macro_rules! fields {
    ($($f:ident = $ty:ident $val:literal),* ) => {
      {
        #[allow(unused_mut)]
        let mut m = HashMap::new();
        $(
          let v = fields!(@VAL $ty $val);
          m.insert(stringify!($f).to_string(), v);
        )*
        m
      }
    };

    (@VAL s $v:literal) => {
      FieldValue::Str(String::from($v))
    };


    (@VAL b $v:literal) => {
      FieldValue::Bool($v)
    };


    (@VAL i $v:literal) => {
      FieldValue::Int($v)
    };

    (@VAL f $v:literal) => {
      FieldValue::Float($v)
    };
  }

  fn events() -> impl Iterator<Item=Event> {
    let kinds = [
      EventKind::SpanCreate,
      EventKind::SpanEnter,
      EventKind::SpanExit,
      EventKind::SpanClose(None),
      EventKind::SpanClose(Some(SpanTime { busy: 1, idle: 20  })),
    ];

    let levels = [
      Level::Trace,
      Level::Debug,
      Level::Info,
      Level::Warn,
      Level::Error,
    ];

    let spans = vec![
      Span {
        name: "egg".to_string(),
        id: NonZeroU64::new(5),
        fields: fields!(q = b false, long = s "a very long string for me")
      },
      Span {
        name: "cat".to_string(),
        id: NonZeroU64::new(6),
        fields: fields!(a = i 4, b= s "bval")
      },
      Span {
        name: "egg".to_string(),
        id: NonZeroU64::new(5),
        fields: fields!(x = f 4.01),      
      }
    ];
    
    let targets = [
      "hey".to_string(),
      "http".to_string(),
      "bad".to_string(),
    ];

    let thread_ids = [
      NonZeroU64::new(0),
      NonZeroU64::new(2),
      NonZeroU64::new(14),
    ];

    let thread_names = [
      None,
      Some("worker".to_string())
    ];

    let src_files = [
      None,
      Some("path/to/code.rs".to_string()),
    ];

    let src_lines = [
      Some(34),
      None
    ];

    let times = [
      None,
      Some(UnixTime::from(Duration::default()))
    ];


    iproduct!(
      kinds,
      levels,
      [spans],
      targets,
      thread_ids,
      thread_names,
      src_files,
      src_lines,
      times
    ).map(|(kind, level, spans, target, thread_id, thread_name, src_file, src_line, time)| Event{
      kind,
      level,
      spans,
      target,
      thread_id,
      thread_name,
      src_file,
      src_line,
      time,
    })
  }


  #[test]
  fn json() {
    let fmt = Json;

    fn deserialize(buf: &[u8]) -> Vec<Event> {
      serde_json::de::Deserializer::from_slice(buf)
      .into_iter()
      .map(Result::unwrap)
      .collect()
    }
      
    test_format(fmt, events(), deserialize);
  }
}