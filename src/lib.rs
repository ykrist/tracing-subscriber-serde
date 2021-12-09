#![cfg_attr(feature = "thread_id", feature(thread_id_value))]

/// `SpanEvent` is re-exported [`FmtEvent`](tracing_subscriber::fmt::format::FmtSpan) from `tracing_subscriber` with
/// a more suitable name.  Implements bitwise arithmetic operations so you can treat it as a set of bitflags.
#[doc(inline)]
pub use tracing_subscriber::fmt::format::FmtSpan as SpanEvents;

mod subscriber;
mod event;
pub mod consumer;
pub mod writer;
pub mod time;
pub mod format;

#[doc(inline)]
pub use subscriber::{SerdeLayerBuilder, SerdeLayer};
#[doc(inline)]
pub use writer::WriteEvent;
#[doc(inline)]
pub use format::SerdeFormat;
#[doc(inline)]
pub use event::{FieldValue, Event, EventKind, Level, Span};

pub use serde;
