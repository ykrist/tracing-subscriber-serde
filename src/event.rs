use crate::time::{SpanTime, UnixTime};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::num::NonZeroU64;

/// The logging level of the event or span.
///
/// The actual [`tracing::Level`] type doesn't implement [`Serialize`] or [`Deserialize`], so this type is used instead.
/// However, it can be freely converted `from` and `into` a [`tracing::Level`].
#[derive(
    Copy, Clone, Debug, Hash, Eq, PartialEq, Serialize_repr, Deserialize_repr, PartialOrd, Ord,
)]
#[repr(u8)]
#[allow(missing_docs)]
pub enum Level {
    Trace = 0,
    Debug = 1,
    Info = 2,
    Warn = 3,
    Error = 4,
}

impl From<tracing::Level> for Level {
    fn from(l: tracing::Level) -> Self {
        match l {
            tracing::Level::TRACE => Level::Trace,
            tracing::Level::DEBUG => Level::Debug,
            tracing::Level::INFO => Level::Info,
            tracing::Level::WARN => Level::Warn,
            tracing::Level::ERROR => Level::Error,
        }
    }
}

impl From<Level> for tracing::Level {
    fn from(l: Level) -> Self {
        match l {
            Level::Trace => tracing::Level::TRACE,
            Level::Debug => tracing::Level::DEBUG,
            Level::Info => tracing::Level::INFO,
            Level::Warn => tracing::Level::WARN,
            Level::Error => tracing::Level::ERROR,
        }
    }
}

/// A tracing value.  `dyn Debug` values are converted to `String` with
/// their `Debug` implementations.
///
/// # Implementation of `Eq`
/// This type implements `Eq`, despite containing a `f64`.  It treats
/// `Float(f64)` as equal to `Float(f64)` if and only if the bit patterns match.
/// This is not the standard handling of `PartialEq` for `f64`, but is designed to be
/// convenient for finding `NaN`s in logs (usually `NaN == NaN` is `false` despite the bit-patterns being identical).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
#[allow(missing_docs)]
pub enum FieldValue {
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
}

#[inline(always)]
fn f64_bitpattern(x: f64) -> u64 {
    // Safety: f64 and u64 have the same size and alignment, and every 64-bit-pattern is
    // valid for u64.
    unsafe { std::mem::transmute::<f64, u64>(x) }
}

impl PartialEq for FieldValue {
    fn eq(&self, other: &FieldValue) -> bool {
        use FieldValue::*;

        match (self, other) {
            (Int(a), Int(b)) => a == b,
            (Bool(a), Bool(b)) => a == b,
            (Str(a), Str(b)) => a == b,
            (Float(a), Float(b)) => f64_bitpattern(*a) == f64_bitpattern(*b),
            _ => false,
        }
    }
}

impl Eq for FieldValue {}

impl Hash for FieldValue {
    fn hash<H: Hasher>(&self, state: &mut H) {
        use FieldValue::*;
        match self {
            Bool(x) => x.hash(state),
            Int(x) => x.hash(state),
            Str(x) => x.hash(state),
            Float(x) => f64_bitpattern(*x).hash(state),
        }
    }
}

macro_rules! impl_field_value_from {
  ($($t:ty => $variant:ident),+ $(,)?) => {
    $(
      impl From<$t> for FieldValue {
        fn from(v: $t) -> FieldValue {
          FieldValue::$variant(v.into())
        }
      }
    )*
  };
}

impl_field_value_from! {
  bool => Bool,
  i64 => Int,
  i32 => Int,
  i16 => Int,
  i8 => Int,
  f32 => Float,
  f64 => Float,
  String => Str,
  &str => Str,
}

/// The type of event which occured
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum EventKind {
    /// A regular tracing event, produced by [`tracing::event!`].
    /// Contains the event fields.  If a message was given to [`tracing::event!`] will be
    /// stored as the `"message"` key in the hashmap.
    Event(IndexMap<String, FieldValue>),
    /// A synthesis event marking the creation of a span
    SpanCreate,
    /// A synthesis event marking the destruction of a span.  If span timings were enabled (see [`SerdeLayerBuilder::with_time_spans`](crate::SerdeLayerBuilder::with_time_spans),
    /// will contain the span timings.
    SpanClose(Option<SpanTime>),
    /// A synthesis event produced when a span is (re-)entered.
    SpanEnter,
    /// A synthesis event produced when a span is exited
    SpanExit,
}

/// The information associated
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Span {
    /// The span's name
    #[serde(alias = "n")]
    pub name: String,

    /// The [span ID](mod@tracing::span), if one was recorded
    #[serde(alias = "i")]
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<NonZeroU64>,

    /// The fields of the span
    #[serde(alias = "f")]
    pub fields: IndexMap<String, FieldValue>,
}

/// A (de)serializable [`tracing`] event.
///
/// If you want to process your stored logs, this is the type you should deserialize.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Event {
    /// The type of event.  
    ///
    /// For a regular, non-synthesised event (i.e `EventKind::Event(_)`), the event
    /// fields can be found in here.
    #[serde(alias = "ty")]
    pub kind: EventKind,

    /// The log level of the event or span.
    #[serde(alias = "l")]
    pub level: Level,

    /// The "callstack" of spans in which the event occurred.  The last element
    /// of `spans` is the inner-most span.
    ///
    /// If `kind` indicates a synthesised span event, the last element in this list contains
    /// the span which was created/destroyed/entered/exited.
    #[serde(alias = "s")]
    pub spans: Vec<Span>,

    /// Target of event, by default the module path in which the event occurred.
    #[serde(alias = "t")]
    pub target: String,

    /// ID of the thread which produced the event
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(alias = "tid")]
    pub thread_id: Option<NonZeroU64>,

    /// Name of the thread which produced the event
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(alias = "tn")]
    pub thread_name: Option<String>,

    /// Line in the source file where the event was produced.
    #[serde(default)]
    #[serde(alias = "srl")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub src_line: Option<u32>,

    /// The source file where the event was produced.
    #[serde(default)]
    #[serde(alias = "srf")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub src_file: Option<String>,

    /// The timestamp of the event.
    #[serde(default)]
    #[serde(alias = "tm")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time: Option<UnixTime>,
}
