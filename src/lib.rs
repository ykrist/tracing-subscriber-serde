#![feature(thread_id_value)]

use std::num::NonZeroU64;
use serde::{Serialize, Deserialize};
use serde_repr::{Serialize_repr, Deserialize_repr};
use tracing::{Subscriber, field::Visit, field::Field, span::{Id, Attributes}, Metadata};
use tracing_subscriber::registry::{LookupSpan, SpanRef, Extensions};
use tracing_subscriber::layer::{Context, Layer};

use std::fmt::{Debug, self, Display};
use std::borrow::Cow;
use crate::time::{UnixTime, Clock, SpanTime, SpanTimer};
use std::rc::Rc;
use smallvec::SmallVec;

mod subscriber;
mod time;

pub use subscriber::JsonLayer;
use std::collections::HashMap;

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
  Float(f64),
  Int(i64),
  Str(String)
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
  pub name: String,
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

// TODO move pretty printer back here

#[cfg(test)]
mod tests {
  use super::*;
  use std::mem::size_of;


}