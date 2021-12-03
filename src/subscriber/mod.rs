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

pub struct JsonLayerBuilder<C, W>(JsonLayer<C, W>);

// TODO : add settings for emitting span enter/exit events, and for span timings
pub struct JsonLayer<C, W> {
  source_location: bool,
  record_span_enter: bool,
  record_span_exit: bool,
  record_span_create: bool,
  record_span_close: bool,
  time_spans: bool,
  writer: W,
  clock: C,
}

impl JsonLayer<(), std::io::Stdout> {
  pub fn new() -> JsonLayerBuilder<(), std::io::Stdout> {
    JsonLayerBuilder(JsonLayer {
      writer: std::io::stdout(),
      clock: (),
      source_location: true,
      record_span_create: false,
      record_span_close: false,
      record_span_enter: false,
      record_span_exit: false,
      time_spans: true,
    })
  }
}

impl<C: Clock, W> JsonLayerBuilder<C, W> {
  pub fn with_clock<C2: Clock>(self, clock: C2) -> JsonLayerBuilder<C2, W> {
    let l = self.0;
    JsonLayerBuilder(JsonLayer{
      source_location: l.source_location,
      record_span_enter: l.record_span_enter,
      record_span_exit: l.record_span_exit,
      record_span_create: l.record_span_create,
      record_span_close: l.record_span_close,
      time_spans: l.time_spans,
      writer: l.writer,
      clock,
    })
  }
  pub fn time_spans(mut self, enable: bool) -> Self {
    self.0.time_spans = enable;
    self
  }

  pub fn span_create(mut self, record: bool) -> Self {
    self.0.record_span_create = record;
    self
  }

  pub fn span_close(mut self, record: bool) -> Self {
    self.0.record_span_close = record;
    self
  }

  pub fn span_enter(mut self, record: bool) -> Self {
    self.0.record_span_enter = record;
    self
  }

  pub fn span_exit(mut self, record: bool) -> Self {
    self.0.record_span_exit = record;
    self
  }


  pub fn source_location(mut self, include: bool) -> Self {
    self.0.source_location = include;
    self
  }

  pub fn finish(mut self) -> JsonLayer<C, W> {
    self.0.record_span_close |= self.0.time_spans;
    self.0
  }
}

impl<T: Clock, W: Write> JsonLayer<T, W> {
  fn emit_event<'a>(&self, meta: &Metadata<'a>, spans: Spans<'a>, e: EventKind<'a>) {
    let thread = std::thread::current();

    let thread_name = thread.name()
      .map(Cow::Borrowed)
      .unwrap_or_else(|| Cow::Owned(format!("{:?}", thread.id())));

    let (src_file, src_line) =
      if self.source_location { (meta.file(), meta.line()) }
      else { (None, None) };

    let event = Event {
      level: (*meta.level()).into(),
      kind: e,
      spans,
      target: meta.target(),
      src_file,
      src_line,
      time: self.clock.get_time(),

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
    let mut extensions = s.extensions_mut();
    let meta = s.metadata();
    let mut spanlist = if self.record_span_create {
      Some(Spans::current(&ctx))
    } else {
      None
    };

    if extensions.get_mut::<Spans>().is_none() {
      let mut span = Spans::default();
      span.new_span(meta.name());
      let mut visitor = FieldVisitor(span);
      attrs.record(&mut visitor);
      let span = visitor.finish();
      if let Some(ref mut spanlist) = spanlist {
        spanlist.append_child(&span);
      }
      extensions.insert(span.clone());
    } else{
      if let Some(ref mut spanlist) = spanlist {
        spanlist.append_child(extensions.get_mut::<Spans>().unwrap());
      }
    }

    if self.time_spans && extensions.get_mut::<SpanTimer>().is_none() {
      extensions.insert(SpanTimer::new());
    }

    if let Some(spanlist) = spanlist.take() {
      self.emit_event(meta, spanlist, EventKind::SpanCreate);
    }
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
    if self.record_span_enter || self.time_spans {
      let s = ctx.span(&id).expect(PANIC_MSG_SPAN_NOT_FOUND);

      if self.record_span_enter {
        let spans = Spans::current(&ctx);
        self.emit_event(s.metadata(), spans, EventKind::SpanEnter);
      }

      if let Some(t) = s.extensions_mut().get_mut::<SpanTimer>() {
        t.start_busy();
      };
    }
  }

  /// Notifies this layer that the span with the given ID was exited.
  fn on_exit(&self, id: &Id, ctx: Context<'_, S>) {
    if self.record_span_exit || self.time_spans {
      let s = ctx.span(id).expect(PANIC_MSG_SPAN_NOT_FOUND);

      if self.record_span_exit {
        let spans = build_leave_span(&ctx, &s);
        self.emit_event(s.metadata(), spans, EventKind::SpanExit);
      }

      if let Some(t) = s.extensions_mut().get_mut::<SpanTimer>() {
        t.end_busy();
      };
    }
  }

  /// Notifies this layer that the span with the given ID has been closed.
  fn on_close(&self, id: Id, ctx: Context<'_, S>) {
    if self.record_span_close {
      let s = ctx.span(&id).expect(PANIC_MSG_SPAN_NOT_FOUND);
      let spans = build_leave_span(&ctx, &s);
      let times = s.extensions().get::<SpanTimer>().map(SpanTimer::finish);
      self.emit_event(s.metadata(), spans, EventKind::SpanClose(times))
    }
  }
}