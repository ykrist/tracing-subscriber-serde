use std::num::NonZeroU64;
use serde::{Serialize, Deserialize};
use serde_repr::{Serialize_repr, Deserialize_repr};
use tracing::{Subscriber, field::Visit, field::Field, span::{Id, Attributes}, Metadata, Span};
use tracing_subscriber::registry::{LookupSpan, SpanRef, Extensions};
use tracing_subscriber::layer::{Context, Layer};

use std::fmt::{Debug, self, Display};
use std::borrow::Cow;

use std::rc::Rc;
use smallvec::SmallVec;
use std::io::Write;

use crate::time::{UnixTime, Clock, SpanTime, SpanTimer};
use crate::FieldValue;
mod serialize;
use serialize::*;

trait AddFields {
  fn add_field(&mut self, name: &'static str, val: FieldValue);
}

struct FieldVisitor<T>(T);

impl<T> FieldVisitor<T> {
  fn finish(self) -> T { self.0 }
}

impl<T: AddFields> Visit for FieldVisitor<T> {
  /// Visit a double-precision floating point value.
  fn record_f64(&mut self, field: &Field, value: f64) {
    self.0.add_field(
      field.name(),
      FieldValue::Float(value)
    )
  }

  /// Visit a signed 64-bit integer value.
  fn record_i64(&mut self, field: &Field, value: i64) {
    self.0.add_field(
      field.name(),
      FieldValue::Int(value)
    )
  }

  /// Visit an unsigned 64-bit integer value.
  fn record_u64(&mut self, field: &Field, value: u64) {
    self.0.add_field(
      field.name(),
      FieldValue::Int(value as i64)
    )
  }

  /// Visit a boolean value.
  fn record_bool(&mut self, field: &Field, value: bool) {
    self.0.add_field(
      field.name(),
      FieldValue::Bool(value)
    )
  }

  /// Visit a string value.
  fn record_str(&mut self, field: &Field, value: &str) {
    self.0.add_field(
      field.name(),
      FieldValue::Str(value.to_string())
    )
  }

  /// Visit a value implementing `fmt::Debug`.
  fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
    self.0.add_field(
      field.name(),
      FieldValue::Str(format!("{:?}", value))
    )
  }
}

// TODO : add settings for emitting span enter/exit events, and for span timings
pub struct JsonLayer<T, W> {
  writer: W,
  timer: T,
}

impl JsonLayer<(), std::io::Stdout> {
  pub fn new() -> Self {
    JsonLayer {
      writer: std::io::stdout(),
      timer: (),
    }
  }
}

impl<T: Clock, W: Write> JsonLayer<T, W> {
  fn emit_event<'a>(&self, meta: &Metadata<'a>, spans: Spans<'a>, e: EventKind<'a>) {
    let thread = std::thread::current();

    let thread_name = thread.name()
      .map(Cow::Borrowed)
      .unwrap_or_else(|| Cow::Owned(format!("{:?}", thread.id())));

    let event = Event {
      level: (*meta.level()).into(),
      kind: e,
      spans,
      target: meta.target(),

      src_file: meta.file(),
      src_line: meta.line(),
      time: self.timer.get_time(),

      thread_id: Some(thread.id().as_u64()),
      thread_name: Some(thread_name.as_ref()),
    };

    let s = serde_json::to_string(&event).unwrap();
    println!("{}", s);
  }
}

const PANIC_MSG_SPAN_NOT_FOUND : &'static str= "bug: span not found";
const PANIC_MSG_SPANS_MISSING : &'static str= "bug: Spans should be in span extensions";

fn build_leave_span<'a, R, S>(ctx: &'a Context<'_, S>, innermost: &SpanRef<'a, R>) -> Spans<'a>
where
  R: LookupSpan<'a>,
  S: Subscriber + for<'l> LookupSpan<'l>
{
  let mut s = Spans::current(ctx);
  s.append_child(innermost.extensions().get().expect(PANIC_MSG_SPANS_MISSING));
  s
}


impl<T, W, S> Layer<S> for JsonLayer<T, W>
  where
    T: Clock + 'static,
    W: Write + 'static,
    S: Subscriber + for<'l> LookupSpan<'l>

{
  fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
    let s = ctx.span(id).expect(PANIC_MSG_SPAN_NOT_FOUND);
    let meta = s.metadata();
    let mut extensions = s.extensions_mut();

    let mut spanlist = Spans::current(&ctx);

    if extensions.get_mut::<Spans>().is_none() {
      let mut span = Spans::default();
      span.new_span(meta.name());
      let mut visitor = FieldVisitor(span);
      attrs.record(&mut visitor);
      let span = visitor.finish();
      spanlist.append_child(&span);
      extensions.insert(span.clone());
    } else{
      spanlist.append_child(extensions.get_mut::<Spans>().unwrap());
    }
    // FIXME store timing info
    self.emit_event(meta, spanlist, EventKind::SpanCreate);
  }


  /// Notifies this layer that an event has occurred.
  fn on_event(&self, event: &tracing::Event<'_>, ctx: Context<'_, S>) {
    let meta = event.metadata();
    let spanlist = Spans::current(&ctx);
    let mut fields = FieldVisitor(EventFields::new());
    event.record(&mut fields);
    let e = EventKind::Event(fields.finish());
    self.emit_event(meta, spanlist, e);
  }

  /// Notifies this layer that a span with the given ID was entered.
  fn on_enter(&self, id: &Id, ctx: Context<'_, S>) {
    let meta = ctx.metadata(id).expect(PANIC_MSG_SPAN_NOT_FOUND);
    let spans = Spans::current(&ctx);
    self.emit_event(meta, spans, EventKind::SpanEnter)
  }

  /// Notifies this layer that the span with the given ID was exited.
  fn on_exit(&self, id: &Id, ctx: Context<'_, S>) {
    let s = ctx.span(id).expect(PANIC_MSG_SPAN_NOT_FOUND);
    let spans = build_leave_span(&ctx, &s);
    self.emit_event(s.metadata(), spans, EventKind::SpanExit)
  }

  /// Notifies this layer that the span with the given ID has been closed.
  fn on_close(&self, id: Id, ctx: Context<'_, S>) {
    let s = ctx.span(&id).expect(PANIC_MSG_SPAN_NOT_FOUND);
    let spans = build_leave_span(&ctx, &s);
    self.emit_event(s.metadata(), spans, EventKind::SpanClose(None))
  }

}