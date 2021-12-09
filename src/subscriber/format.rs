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
/// Below is the implementation for [`JsonFormat`].
/// ```ignore
/// impl SerdeFormat for JsonFormat {
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
pub struct JsonFormat;

impl SerdeFormat for JsonFormat {
  fn message_size_hint(&self) -> usize { 512 }

  fn serialize(&self, mut buf: impl Write, event: impl Serialize) -> std::io::Result<()> {
    serde_json::to_writer(&mut buf, &event)?;
    buf.write("\n".as_bytes())?;
    Ok(())
  }
}
