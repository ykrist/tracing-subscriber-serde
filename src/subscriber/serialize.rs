use super::*;
use serde::Serializer;
use serde::ser::{SerializeMap, SerializeSeq};
use crate::{Level};
use super::SString;

#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub enum FieldValue {
  Bool(bool),
  Float(f64),
  Int(i64),
  Str(SString)
}



#[derive(Debug, Clone, Serialize)]
#[serde(rename_all="snake_case")]
pub enum EventKind<'a> {
  #[serde(serialize_with="serialize_event_fields")]
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
  Start { span_name: &'a str, id: Option<NonZeroU64> },
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

  pub fn new_span(&mut self, span_meta: &Metadata, span_id: Option<NonZeroU64>) {
    self.0.reserve(span_meta.fields().len() + 1);
    self.0.push(SpanItem::Start { span_name: span_meta.name(), id: span_id });
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


fn serialize_event_fields<S>(fields: &EventFields, s: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer
{
  let mut m = s.serialize_map(Some(fields.len()))?;
  for (field, val) in fields {
    m.serialize_entry(field, val)?;
  }
  m.end()
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
      SpanItem::Start{ span_name, id } => {
        m.serialize_entry("n", span_name)?;
        if let Some(id) = id {
          m.serialize_entry("i", id)?;
        }
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
        if matches!(item, SpanItem::Start{..}) {
          seq.serialize_element(&SerializeSpan(&items[start..next_start]))?;
          start = next_start;
        }
      }
      seq.serialize_element(&SerializeSpan(&items[start..]))?;
    }
    seq.end()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn float_eq(a: f64, b: f64) -> bool {
    // Safety: f64 and u64 have the same size and alignment, and every 64-bit-pattern is
    // valid for u64.
    unsafe {
      let a : u64 = std::mem::transmute(a);
      let b : u64 = std::mem::transmute(b);
      a == b
    }
  }

  fn eq_field_values(a: &crate::FieldValue, b: &FieldValue) -> bool {
    use crate::FieldValue::*;

    match (a, b) {
      (Int(a), FieldValue::Int(b)) => a == b,
      (Bool(a), FieldValue::Bool(b)) => a == b,
      (Float(a), FieldValue::Float(b)) => float_eq(*a, *b),
      (Str(a), FieldValue::Str(b)) => a == b,
      _ => false,
    }
  }

  macro_rules! assert_field_values_eq {
      ($left:expr, $right:expr) => {
        let left = &$left;
        let right = &$right;
        if !eq_field_values(left, right) {
          panic!("assert eq failed: {:?} != {:?}", left, right)
        }
      };
  }

  // TODO should probably fuzz this
  #[test]
  fn serde_borrowed_to_owned() {
    let e = Event {
      kind: EventKind::Event(smallvec::smallvec![
        ("message", FieldValue::Str("oh no!".into())),
        ("x", FieldValue::Int(42)),
      ]),
      level: Level::Trace,
      spans: Spans(vec![
        SpanItem::Start{ span_name: "hello_world", id: NonZeroU64::new(1) },
        SpanItem::Field{ name: "field", val: FieldValue::Bool(false) },
      ]),
      target: "foo",
      thread_id: NonZeroU64::new(1),
      thread_name: Some("WorkerThread"),
      src_line: Some(20),
      src_file: Some("src/module/file.rs"),
      time: Some(UnixTime{ seconds: 10, nanos: 11 })
    };


    let serialized = serde_json::to_string_pretty(&e).unwrap();
    println!("{}", serialized);

    let de : crate::Event = serde_json::from_str(&serialized).unwrap();

    assert!(matches!(&de.kind, crate::EventKind::Event(_)));
    match &de.kind {
      crate::EventKind::Event(fields) => {
        assert_eq!(fields.len(), 2);
        assert_field_values_eq!(fields["message"], FieldValue::Str("oh no!".into()));
        assert_field_values_eq!(fields["x"], FieldValue::Int(42));
      },
      other => panic!("wrong event kind: {:?}", other)
    }

    assert_eq!(de.level, crate::Level::Trace);
    assert_eq!(de.spans.len(), 1);
    let span = &de.spans[0];
    assert_eq!(&span.name, "hello_world");
    assert_eq!(&span.id, &NonZeroU64::new(1));
    assert_eq!(span.fields.len(), 1);
    assert_field_values_eq!(span.fields["field"], FieldValue::Bool(false));

    assert_eq!(de.target, "foo");
    assert_eq!(de.thread_id, NonZeroU64::new(1));
    assert_eq!(de.thread_name, Some("WorkerThread".to_string()));
    assert_eq!(de.src_file, Some("src/module/file.rs".to_string()));
    assert_eq!(de.time, Some(UnixTime{ seconds: 10, nanos: 11}));
  }
}