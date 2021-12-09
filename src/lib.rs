#![cfg_attr(feature = "thread_id", feature(thread_id_value))]
#![warn(missing_docs)]
// only enables the `doc_cfg` feature when
// the `docsrs` configuration attribute is defined
#![cfg_attr(docsrs, feature(doc_cfg))]

//! hello there
//!
//! | Feature | Default enabled? | Description | Dependencies |
//! | --- | --- | --- | --- |
//! | `thread_id` | No | Enable recording thread IDs in events | [`thread_id_value`](https://github.com/rust-lang/rust/issues/67939) unstable feature |
//! | `consumer` | Yes | Consumer API for pretty-printing events | [`ansi_term`] crate |
//!


/// `SpanEvent` is re-exported [`FmtEvent`](tracing_subscriber::fmt::format::FmtSpan) from `tracing_subscriber` with
/// a more suitable name.  Implements bitwise arithmetic operations so you can treat it as a set of bitflags.
#[doc(inline)]
pub use tracing_subscriber::fmt::format::FmtSpan as SpanEvents;

mod subscriber;
mod event;

//
#[cfg_attr(docsrs, doc(cfg(feature = "consumer")))]
#[cfg(any(feature = "consumer"))]
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
