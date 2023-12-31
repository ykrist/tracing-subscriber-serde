use super::SString;
use super::*;
use crate::Level;
use serde::ser::{SerializeMap, SerializeSeq};
use serde::Serializer;

#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub enum FieldValue {
    Bool(bool),
    Float(f64),
    Int(i64),
    Str(SString),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind<'a> {
    #[serde(serialize_with = "serialize_event_fields")]
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
    #[serde(rename = "ty")]
    pub kind: EventKind<'a>,
    #[serde(rename = "l")]
    pub level: Level,
    #[serde(rename = "s")]
    pub spans: Spans<'a>,

    #[serde(rename = "t")]
    pub target: &'a str,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "tid")]
    pub thread_id: Option<NonZeroU64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "tn")]
    pub thread_name: Option<&'b str>,

    #[serde(rename = "srl")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub src_line: Option<u32>,

    #[serde(rename = "srf")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub src_file: Option<&'a str>,

    #[serde(rename = "tm")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time: Option<UnixTime>,
}

#[derive(Debug, Clone)]
pub enum SpanItem<'a> {
    Start {
        span_name: &'a str,
        id: Option<NonZeroU64>,
    },
    Field {
        name: &'a str,
        val: FieldValue,
    },
}

#[derive(Default, Clone, Debug)]
pub struct Spans<'a>(Vec<SpanItem<'a>>);

impl<'a> Spans<'a> {
    pub fn current<S>(ctx: &'a Context<'_, S>) -> Self
    where
        S: Subscriber + for<'l> LookupSpan<'l>,
    {
        let mut spans = ctx
            .lookup_current()
            .into_iter()
            .map(|s| s.scope().from_root())
            .flatten();

        let mut spanlist = match spans.next() {
            Some(s) => s
                .extensions()
                .get::<Spans>()
                .expect(PANIC_MSG_SPANS_MISSING)
                .clone(),
            None => return Self::default(),
        };

        for s in spans {
            spanlist.append_child(
                s.extensions()
                    .get::<Spans>()
                    .expect(PANIC_MSG_SPANS_MISSING),
            );
        }

        spanlist
    }

    pub fn new_span(&mut self, span_meta: &Metadata, span_id: Option<NonZeroU64>) {
        self.0.reserve(span_meta.fields().len() + 1);
        self.0.push(SpanItem::Start {
            span_name: span_meta.name(),
            id: span_id,
        });
    }

    pub fn append_child(&mut self, child: &Self) {
        self.0.extend_from_slice(&child.0)
    }

    #[allow(dead_code)]
    pub fn as_items(&self) -> &[SpanItem] {
        &*self.0
    }
}

impl<'a> AddFields for Spans<'a> {
    fn add_field(&mut self, name: &'static str, val: FieldValue) {
        self.0.push(SpanItem::Field { name, val });
    }
}

fn serialize_event_fields<S>(fields: &EventFields, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
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
                SpanItem::Field { name, val } => m.serialize_entry(name, val)?,
                _ => unreachable!(),
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
        let mut m = serializer.serialize_map(Some(3))?;
        let name = &(self.0)[0];
        let fields = SerializeSpanFields(&(self.0)[1..]);

        match name {
            SpanItem::Start { span_name, id } => {
                m.serialize_entry("n", span_name)?;
                if let Some(id) = id {
                    m.serialize_entry("i", id)?;
                }
            }
            _ => unreachable!(),
        }
        m.serialize_entry("f", &fields)?;
        m.end()
    }
}

impl Serialize for Spans<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let items = self.0.as_slice();
        let len = items
            .iter()
            .filter(|i| matches!(i, SpanItem::Start { .. }))
            .count();
        let mut seq = serializer.serialize_seq(Some(len))?;

        if !items.is_empty() {
            let mut start = 0;
            for (next_start, item) in items.iter().enumerate().skip(1) {
                if matches!(item, SpanItem::Start { .. }) {
                    seq.serialize_element(&SerializeSpan(&items[start..next_start]))?;
                    start = next_start;
                }
            }
            seq.serialize_element(&SerializeSpan(&items[start..]))?;
        }
        seq.end()
    }
}

#[cfg(all(test, feature = "consumer"))]
mod tests {
    use super::*;
    use crate::consumer::*;
    use crate::test_utils::*;

    // TODO: should probably fuzz this
    fn serde_borrowed_to_owned<F>(fmt: F)
    where
        F: SerdeFormat + for<'a> StreamFormat<&'a [u8]>,
    {
        let e = Event {
            kind: EventKind::Event(smallvec::smallvec![
                ("message", FieldValue::Str("oh no!".into())),
                ("x", FieldValue::Int(42)),
            ]),
            level: Level::Trace,
            spans: Spans(vec![
                SpanItem::Start {
                    span_name: "hello_world",
                    id: NonZeroU64::new(1),
                },
                SpanItem::Field {
                    name: "field",
                    val: FieldValue::Bool(false),
                },
            ]),
            target: "foo",
            thread_id: NonZeroU64::new(1),
            thread_name: Some("WorkerThread"),
            src_line: Some(20),
            src_file: Some("src/module/file.rs"),
            time: Some(UnixTime {
                seconds: 10,
                nanos: 11,
            }),
        };

        let mut buf = Vec::new();
        fmt.serialize(&mut buf, &e).unwrap();
        println!("serialized:");
        for &byte in &buf {
            print!("{}", std::ascii::escape_default(byte))
        }
        println!();

        let mut stream = fmt.iter_reader(&*buf);
        let de = stream.next().unwrap().unwrap();

        if !eq_event_ser_event(&de, &e) {
            eprintln!("  serialized = {:?}", &e);
            eprintln!("deserialized = {:?}", &de);
            panic!("serialization/deserialization mismatch")
        }

        assert!(stream.next().is_none());
    }

    #[test]
    fn serde_borrowed_to_owned_json() {
        serde_borrowed_to_owned(Json);
    }

    #[cfg(feature = "messagepack")]
    #[test]
    fn serde_borrowed_to_owned_msgpack() {
        serde_borrowed_to_owned(crate::format::MessagePack);
    }
}
