//! Serde formats and supporting trait.
use serde::Serialize;
use std::io::Write;

mod json;
pub use json::Json;
#[cfg(feature = "consumer")]
pub use json::JsonStream;

#[cfg(feature = "messagepack")]
mod messagepack;
#[cfg(feature = "messagepack")]
#[cfg_attr(docsrs, doc(cfg(feature = "messagepack")))]
pub use messagepack::MessagePack;
#[cfg(all(feature = "messagepack", feature = "consumer"))]
pub use messagepack::MessagePackStream;

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
    fn message_size_hint(&self) -> usize {
        T::message_size_hint(self)
    }

    fn serialize(&self, buf: impl Write, event: impl Serialize) -> std::io::Result<()> {
        T::serialize(self, buf, event)
    }
}

#[cfg(all(test, feature = "consumer"))]
mod tests {
    use super::*;
    use crate::consumer::*;
    use crate::test_utils::eq_event;
    use crate::{
        time::{SpanTime, UnixTime},
        Event, EventKind, FieldValue, Level, Span,
    };
    use itertools::iproduct;
    use std::collections::HashMap;
    use std::{num::NonZeroU64, time::Duration};

    pub(super) fn test_format<F>(fmt: F)
    where
        F: SerdeFormat + for<'a> StreamFormat<&'a [u8]>,
    {
        let mut events = Vec::new();
        let mut buffer = Vec::new();

        // let mut data_len = 0;
        for e in self::events() {
            fmt.serialize(&mut buffer, &e).unwrap();
            events.push(e);
            // eprintln!("serialized {} bytes", buffer.len() - data_len);
            // data_len = buffer.len();
        }

        let mut deserialized = Vec::with_capacity(events.len());
        for e in fmt.iter_reader(&buffer) {
            deserialized.push(e.unwrap());
        }

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
            let mut m = indexmap::IndexMap::new();
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

    fn events() -> impl Iterator<Item = Event> {
        let kinds = [
            EventKind::SpanCreate,
            EventKind::SpanEnter,
            EventKind::SpanExit,
            EventKind::SpanClose(None),
            EventKind::SpanClose(Some(SpanTime { busy: 1, idle: 20 })),
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
                fields: fields!(q = b false, long = s "a very long string for me"),
            },
            Span {
                name: "cat".to_string(),
                id: NonZeroU64::new(6),
                fields: fields!(a = i 4, b= s "bval"),
            },
            Span {
                name: "egg".to_string(),
                id: NonZeroU64::new(5),
                fields: fields!(x = f 4.01),
            },
        ];

        let targets = ["hey".to_string(), "http".to_string(), "bad".to_string()];

        let thread_ids = [NonZeroU64::new(0), NonZeroU64::new(2), NonZeroU64::new(14)];

        let thread_names = [None, Some("worker".to_string())];

        let src_files = [None, Some("path/to/code.rs".to_string())];

        let src_lines = [Some(34), None];

        let times = [None, Some(UnixTime::from(Duration::default()))];

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
        )
        .map(
            |(kind, level, spans, target, thread_id, thread_name, src_file, src_line, time)| {
                Event {
                    kind,
                    level,
                    spans,
                    target,
                    thread_id,
                    thread_name,
                    src_file,
                    src_line,
                    time,
                }
            },
        )
    }
}
