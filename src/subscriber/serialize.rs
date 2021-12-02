use super::*;
use serde::Serializer;
use serde::ser::{SerializeMap, SerializeSeq};
use crate::{FieldValue, Level};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all="snake_case")]
pub enum EventKind<'a> {
  #[serde(borrow)]
  Event(EventFields<'a>),
  SpanCreate,
  SpanClose(Option<SpanTime>),
  SpanEnter,
  SpanExit,
}

pub type EventFields<'a> = SmallVec<[(&'a str, FieldValue); 8]>;

impl<'a> AddFields for EventFields<'a> {
  fn add_field(&mut self, name: &'static str, val: FieldValue) {
    self.push((name, val))
  }
}

#[derive(Clone, Debug, Serialize)]
pub struct Event<'a, 'b> {
  #[serde(rename="ty")]
  pub kind: EventKind<'a>,
  #[serde(rename="l")]
  pub level: Level,
  #[serde(rename="s")]
  pub spans: Spans<'a>,

  #[serde(rename="t")]
  pub target: &'a str,

  #[serde(skip_serializing_if="Option::is_none")]
  #[serde(rename="tid")]
  pub thread_id: Option<NonZeroU64>,

  #[serde(skip_serializing_if="Option::is_none")]

  #[serde(rename="tn")]
  pub thread_name: Option<&'b str>,

  #[serde(rename="srl")]
  #[serde(skip_serializing_if="Option::is_none")]
  pub src_line: Option<u32>,

  #[serde(rename="srf")]
  #[serde(skip_serializing_if="Option::is_none")]
  pub src_file: Option<&'a str>,

  #[serde(rename="tm")]
  #[serde(skip_serializing_if="Option::is_none")]
  pub time: Option<UnixTime>
}


#[derive(Debug, Clone)]
enum SpanItem<'a> {
  Name(&'a str),
  Field{ name: &'a str, val: FieldValue }
}


#[derive(Default, Clone, Debug)]
pub struct Spans<'a>(Vec<SpanItem<'a>>);

impl<'a> Spans<'a> {
  pub fn current<S>(ctx: &'a Context<'_, S>) -> Self
    where
      S: Subscriber + for<'l> LookupSpan<'l>
  {

    let mut spans = ctx.lookup_current()
      .into_iter()
      .map(|s| s.scope().from_root())
      .flatten();

    let mut spanlist = match spans.next() {
      Some(s) => s.extensions().get::<Spans>().expect(PANIC_MSG_SPANS_MISSING).clone(),
      None => return Self::default(),
    };

    for s in spans {
      spanlist.append_child(s.extensions().get::<Spans>().expect(PANIC_MSG_SPANS_MISSING));
    }

    spanlist
  }

  pub fn new_span(&mut self, name: &'a str) {
    self.0.push(SpanItem::Name(name));
  }


  pub fn append_child(&mut self, child: &Self) {
    self.0.extend_from_slice(&child.0)
  }
}

impl<'a> AddFields for Spans<'a> {
  fn add_field(&mut self, name: &'static str, val: FieldValue) {
    self.0.push(SpanItem::Field{name, val});
  }
}


struct SerializeSpanFields<'a>(&'a [SpanItem<'a>]);

impl Serialize for SerializeSpanFields<'_> {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
      S: Serializer,
  {
    let mut m = serializer.serialize_map(Some(self.0.len()))?;
    for e in self.0 {
      match e {
        SpanItem::Field { name, val} => m.serialize_entry(name, val)?,
        _ => unreachable!()
      }
    }
    m.end()
  }
}

struct SerializeSpan<'a>(&'a [SpanItem<'a>]);

impl Serialize for SerializeSpan<'_> {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
      S: Serializer,
  {
    let mut m = serializer.serialize_map(Some(self.0.len()))?;
    let name = &(self.0)[0];
    let fields = &(self.0)[1..];

    match name {
      SpanItem::Name(n) => {
        m.serialize_entry("n", n)?;
      }
      _ => unreachable!(),
    }
    m.serialize_entry("f", &SerializeSpanFields(fields))?;
    m.end()
  }
}

impl Serialize for Spans<'_> {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
      S: Serializer,
  {
    let items = self.0.as_slice();
    let mut seq = serializer.serialize_seq(None)?;

    if !items.is_empty() {
      let mut start = 0;
      for (next_start, item) in items.iter().enumerate().skip(1) {
        if matches!(item, SpanItem::Name(_)) {
          seq.serialize_element(&SerializeSpan(&items[start..next_start]))?;
          start = next_start;
        }
      }
      seq.serialize_element(&SerializeSpan(&items[start..]))?;
    }
    seq.end()
  }
}


