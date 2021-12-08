#![cfg_attr(feature = "thread_id", feature(thread_id_value))]

use std::num::NonZeroU64;
use serde::{Serialize, Deserialize};
use serde_repr::{Serialize_repr, Deserialize_repr};

use std::fmt::{Debug};

pub mod subscriber;
pub use subscriber::SerdeLayer;
pub use tracing_subscriber::fmt::format::FmtSpan;

pub mod time;
use crate::time::{UnixTime, SpanTime};

use std::collections::HashMap;

pub mod consumer;

mod nonblocking;
pub use nonblocking::{nonblocking, WriteRecord, FlushGuard};

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
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

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FieldValue {
  Bool(bool),
  Int(i64),
  Float(f64),
  Str(String)
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


#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all="snake_case")]
pub enum EventKind {
  Event(HashMap<String, FieldValue>),
  SpanCreate,
  SpanClose(Option<SpanTime>),
  SpanEnter,
  SpanExit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Span {
  #[serde(alias="n")]
  pub name: String,
  #[serde(alias="f")]
  pub fields: HashMap<String, FieldValue>
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
  #[serde(alias="ty")]
  pub kind: EventKind,

  #[serde(alias="l")]
  pub level: Level,

  #[serde(alias="s")]
  pub spans: Vec<Span>,

  #[serde(alias="t")]
  pub target: String,

  #[serde(default)]
  #[serde(skip_serializing_if="Option::is_none")]
  #[serde(alias="tid")]
  pub thread_id: Option<NonZeroU64>,

  #[serde(default)]
  #[serde(skip_serializing_if="Option::is_none")]
  #[serde(alias="tn")]
  pub thread_name: Option<String>,

  #[serde(default)]
  #[serde(alias="srl")]
  #[serde(skip_serializing_if="Option::is_none")]
  pub src_line: Option<u32>,

  #[serde(default)]
  #[serde(alias="srf")]
  #[serde(skip_serializing_if="Option::is_none")]
  pub src_file: Option<String>,

  #[serde(default)]
  #[serde(alias="tm")]
  #[serde(skip_serializing_if="Option::is_none")]
  pub time: Option<UnixTime>
}
