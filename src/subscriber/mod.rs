use std::num::NonZeroU64;
use std::fmt::{Debug, self, Write as FmtWrite};
use std::io::{Stdout};
use std::borrow::Cow;

use serde::{Serialize};
use tracing::{Subscriber, field::Visit, field::Field, span::{Id, Attributes}, Metadata};
use tracing_subscriber::registry::{LookupSpan, SpanRef};
use tracing_subscriber::layer::{Context, Layer};

use smallvec::SmallVec;
use smartstring::alias::String as SString;

use crate::time::{UnixTime, Clock, SpanTime, SpanTimer};
use crate::{SpanEvents, WriteEvent, SerdeFormat};
use crate::format::Json;

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
      FieldValue::Str(value.into())
    )
  }

  /// Visit a value implementing `fmt::Debug`.
  fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
    let mut s = SString::new();
    write!(&mut s, "{:?}", value).unwrap();
    self.0.add_field(
      field.name(),
      FieldValue::Str(s)
    )
  }
}

/// Configuration for [`SerdeLayer`]
pub struct SerdeLayerBuilder<F, C, W> {
  source_location: bool,
  span_events: SpanEvents,
  time_spans: bool,
  fmt: F,
  writer: W,
  clock: C,
  thread_name: bool,
  thread_id: bool,
}

/// A tracing-subscriber [`Layer`](tracing_subscriber::Layer) which serializes events to any
///  [serde-supported format](https://docs.rs/serde)
///
/// The events can be later deserialized using serde with the provided [`Event`](crate::Event) type.
///
/// The layer is parameterised by three types:
/// - `F` : the [`SerdeFormat`] you want to use, the default is [`Json`].
/// - `C` : the [`Clock`] used to optionally produce timestamps.  The default is `()`, which is no clock.
/// - `W` : the [`WriteEvent`] writer used for output.
///
/// See [`SerdeLayerBuilder`] for details on configuration and options.
pub struct SerdeLayer<F, C, W> {
  thread_name: bool,
  #[cfg_attr(not(feature = "thread_id"), allow(dead_code))]
  thread_id: bool,
  source_location: bool,
  record_span_enter: bool,
  record_span_exit: bool,
  record_span_create: bool,
  record_span_close: bool,
  time_spans: bool,
  fmt: F,
  writer: W,
  clock: C,
}




impl SerdeLayer<Json, (), Stdout> {
  /// Start building a new layer.
  pub fn new() -> SerdeLayerBuilder<Json, (), Stdout> {
    SerdeLayerBuilder {
      thread_name: false,
      thread_id: false,
      writer: std::io::stdout(),
      clock: (),
      fmt: Json,
      source_location: true,
      time_spans: false,
      span_events: SpanEvents::NONE
    }
  }
}

impl<F, C, W> SerdeLayerBuilder<F, C, W>
where
  F: SerdeFormat,
  C: Clock,
  W: WriteEvent,
{
  /// Output events using a given [`WriteEvent`] writer.
  ///
  /// See the trait documentation on how to implement.
  ///
  /// Will accept [`Stdout`], [`Stdout`], [`NonBlocking`](crate::writer::NonBlocking)
  /// and [`Mutex<W>`](std::sync::Mutex) where `W: io::Write`.
  pub fn with_writer<W2>(self, writer: W2) -> SerdeLayerBuilder<F, C, W2>
    where
      W2: WriteEvent
  {
    SerdeLayerBuilder {
      thread_name: self.thread_name,
      thread_id: self.thread_id,
      source_location: self.source_location,
      span_events: self.span_events,
      time_spans: self.time_spans,
      writer,
      fmt: self.fmt,
      clock: self.clock,
    }
  }

  /// Use the supplied [`Clock`] to produce timestamps.
  ///
  /// Span timings (busy/idle) will use [`std::time::Instant`] regardless
  /// of the clock, this only affects the `timestamp` field of [`Event`](crate::Event).
  pub fn with_clock<C2: Clock>(self, clock: C2) -> SerdeLayerBuilder<F, C2, W> {
    SerdeLayerBuilder {
      thread_name: self.thread_name,
      thread_id: self.thread_id,
      source_location: self.source_location,
      span_events: self.span_events,
      time_spans: self.time_spans,
      writer: self.writer,
      fmt: self.fmt,
      clock,
    }
  }

  /// Time spans' busy and idle time using [`std::time::Instant`].
  ///
  /// Enabling this will always enable [`SpanEvents::CLOSE`]. Disabled by default.
  ///
  /// This will be stored in [`EventKind::SpanClose`](crate::EventKind::SpanClose) in the `kind` field of [`Event`](crate::Event).
  pub fn with_time_spans(mut self, enable: bool) -> Self {
    self.time_spans = enable;
    self
  }

  /// Control the output of synthesised events when spans
  /// are constructed/entered and destroyed/exited.
  pub fn with_span_events(mut self, e: SpanEvents) -> Self {
    self.span_events = e;
    self
  }

  /// Record thread information (names and thread IDs).  Logging thread IDs requires the `thread_id`
  /// feature which is only available on the Nightly compiler.
  pub fn with_thread_info(mut self, names: bool, ids: bool) -> Self {
    self.thread_name = names;
    self.thread_id = ids;
    self
  }

  /// Record the line number and source file of the event .
  pub fn with_source_location(mut self, include: bool) -> Self {
    self.source_location = include;
    self
  }

  /// Finish configuration.
  pub fn finish(self) -> SerdeLayer<F, C, W> {
    macro_rules! bit_is_set {
        ($x:expr, $bit:path) => {
          $x.clone() & $bit.clone() == $bit.clone()
        };
    }

    SerdeLayer {
      record_span_create: bit_is_set!(self.span_events, SpanEvents::NEW),
      record_span_close: bit_is_set!(self.span_events, SpanEvents::CLOSE) || self.time_spans,
      record_span_enter: bit_is_set!(self.span_events, SpanEvents::ENTER),
      record_span_exit: bit_is_set!(self.span_events, SpanEvents::EXIT),
      thread_id: self.thread_id,
      thread_name: self.thread_name,
      source_location: self.source_location,
      time_spans: self.time_spans,
      writer: self.writer,
      clock: self.clock,
      fmt: self.fmt,
    }
  }
}

impl<F, C, W> SerdeLayer<F, C, W>
  where
    F: SerdeFormat,
    C: Clock,
    W: WriteEvent,
{
  fn emit_event<'a>(&self, meta: &Metadata<'a>, spans: Spans<'a>, e: EventKind<'a>) {
    let thread = std::thread::current();

    let thread_name = thread.name()
      .map(Cow::Borrowed)
      .unwrap_or_else(|| Cow::Owned(format!("{:?}", thread.id())));

    let (src_file, src_line) =
      if self.source_location { (meta.file(), meta.line()) }
      else { (None, None) };

    #[cfg(feature = "thread_id")]
    let thread_id = if self.thread_id { Some(thread.id().as_u64()) } else { None };
    #[cfg(not(feature = "thread_id"))]
    let thread_id = None;

    let thread_name = if self.thread_name { Some(thread_name.as_ref()) } else { None };

    let event = Event {
      level: (*meta.level()).into(),
      kind: e,
      spans,
      target: meta.target(),
      src_file,
      src_line,
      time: self.clock.time(),
      thread_id,
      thread_name,
    };

    // If users want their errors handled they can choose themselves
    // using a wrapper type over their chosen WriteRecord
    let _ = self.writer.write(&self.fmt, &event);
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


impl<F, C, W, S> Layer<S> for SerdeLayer<F, C, W>
    where
    F: SerdeFormat + 'static,
    C: Clock + 'static,
    W: WriteEvent + 'static,
    S: Subscriber + for<'l> LookupSpan<'l>,
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

