use ansi_term::{Colour};
use crate::{Event, Level, FieldValue, EventKind, Span};
use std::fmt::{Display, Formatter, Result as FmtResult};
use std::time::Duration;

/// Configuration of pretty formatting for events.
#[derive(Debug, Copy, Clone)]
pub struct PrettyPrinter {
  source: bool,
  target: bool,
  span_times: bool,
  limit_spans: usize,
  continue_line: &'static str,
}

/// A formatted event which implements [`Display`].
#[derive(Debug, Copy, Clone)]
pub struct FmtEvent<'a> {
  printer: &'a PrettyPrinter,
  event: &'a Event,
}


#[derive(Debug, Copy, Clone)]
struct FmtSpan<'a> {
  printer: &'a PrettyPrinter,
  span: &'a Span,
}

impl Default for PrettyPrinter {
  fn default() -> Self {
    PrettyPrinter {
      source: true,
      target: true,
      span_times: true,
      limit_spans: usize::MAX,
      continue_line: "  | ",
    }
  }
}

impl PrettyPrinter {
  pub fn limit_spans(mut self, limit: usize) -> Self {
    self.limit_spans = limit;
    self
  }

  pub fn show_source(mut self, on: bool) -> Self {
    self.source = on;
    self
  }

  pub fn show_target(mut self, on: bool) -> Self {
    self.target = on;
    self
  }

  pub fn show_span_times(mut self, on: bool) -> Self {
    self.span_times = on;
    self
  }
}


impl Display for FmtEvent<'_> {
  fn fmt(&self, f: &mut Formatter) -> FmtResult {
    let lvl = match self.event.level {
      Level::Trace => Colour::Purple.bold().paint("TRACE"),
      Level::Debug => Colour::Green.bold().paint("DEBUG"),
      Level::Info => Colour::Blue.bold().paint(" INFO"),
      Level::Warn => Colour::Yellow.bold().paint(" WARN"),
      Level::Error => Colour::Red.bold().paint("ERROR"),
    };

    f.write_fmt(format_args!("{}: ", lvl))?;

    let mut spans = self.event.spans.iter().rev().take(self.printer.limit_spans);

    match &self.event.kind {
      EventKind::Event(fields) => {
        if let Some(msg) = fields.get("message") {
          self.printer.fmt_fieldvalue(f, msg)?;
          if fields.len() > 1 {
            f.write_str("\n")?;
            f.write_str(self.printer.continue_line)?;
            self.printer.fmt_fields(f, fields.iter().filter(|(n, _)| n.as_str() != "message"))?;
          }
        } else {
          self.printer.fmt_fields(f, fields.iter())?;
        }
        f.write_str("\n")?;
      }

      kind => {
        if let Some(span) = spans.next() {
          write!(f, "{} ", self.printer.fmt_span(span))?;
        }

        let verb = match kind {
          EventKind::Event(_) => unreachable!(),
          EventKind::SpanExit => "exit",
          EventKind::SpanEnter => "enter",
          EventKind::SpanClose(_) => "close",
          EventKind::SpanCreate => "create",
        };

        let verb = Colour::Cyan.underline().paint(verb);

        match kind {
          EventKind::SpanClose(Some(times)) if self.printer.span_times => {
            let busy = Duration::from_nanos(times.busy());
            let idle = Duration::from_nanos(times.idle());
            write!(f, "{}: {:?} busy, {:?} idle\n", verb, busy, idle)?;
          },
          _ => {
            write!(f, "{}\n", verb)?;
          }
        }

      }
    }

    for span in spans {
      write!(f, "{}in {}\n", self.printer.continue_line, self.printer.fmt_span(span))?;
    }


    if self.printer.target || self.printer.source {
      f.write_str(self.printer.continue_line)?;

      if self.printer.target {
        f.write_fmt(format_args!(
          "{} {} ",
          Colour::White.italic().paint("target"),
          Colour::White.bold().paint(&self.event.target)))?;
      }

      if self.printer.source {
        if let Some(file) = self.event.src_file.as_ref() {
          f.write_fmt(format_args!("{} {}", Colour::White.italic().paint("at"), file))?;
          if let Some(lineno) = self.event.src_line {
            f.write_fmt(format_args!(":{}", lineno))?;
          }
        }
      }
      f.write_str("\n")?;
    }

    Ok(())
  }
}


impl Display for FmtSpan<'_> {
  fn fmt(&self, f: &mut Formatter) -> FmtResult {
    f.write_fmt(format_args!(
      "{}{{",
      Colour::White.bold().paint(&self.span.name),
    ))?;
    self.printer.fmt_fields(f, &self.span.fields)?;
    f.write_str("}")?;
    Ok(())
  }
}

impl PrettyPrinter {
  pub fn fmt<'a>(&'a self, event: &'a Event) -> FmtEvent<'a> {
    FmtEvent { printer: self, event }
  }

  pub fn print(&self, event: &Event) {
    println!("{}", self.fmt(event));
  }

  fn fmt_span<'a>(&'a self, span: &'a Span) -> FmtSpan<'a> {
    FmtSpan { printer: self, span }
  }

  fn fmt_fieldvalue(&self, f: &mut Formatter, v: &FieldValue) -> FmtResult {
    match v {
      FieldValue::Int(n) => f.write_fmt(format_args!("{}", Colour::Purple.paint(format!("{}", n))))?,
      FieldValue::Float(v) => f.write_fmt(format_args!("{}", Colour::Purple.paint(format!("{}", v))))?,
      FieldValue::Bool(v) => f.write_fmt(format_args!("{}", Colour::Yellow.paint(format!("{}", v))))?,
      FieldValue::Str(v) => f.write_fmt(format_args!("{}", v))?,
    };
    Ok(())
  }

  fn fmt_field(&self, f: &mut Formatter, field: (&str, &FieldValue)) -> FmtResult {
    f.write_fmt(format_args!("{}= ", Colour::Blue.paint(field.0)))?;
    self.fmt_fieldvalue(f, field.1)
  }

  fn fmt_fields<'a, S, I>(&'a self, f: &mut Formatter, fields: I) -> FmtResult
    where
      S: AsRef<str> + 'a,
      I: IntoIterator<Item=(&'a S, &'a FieldValue)> + 'a
  {
    let mut fields = fields.into_iter().map(|(f, v)| (f.as_ref(), v));
    if let Some(field) = fields.next() {
      self.fmt_field(f, field)?;
    }
    for field in fields {
      f.write_str(", ")?;
      self.fmt_field(f, field)?;
    }
    Ok(())
  }
}


#[cfg(test)]
mod tests {
  use super::*;
  use crate::consumer::iter_json_file;


  #[test]
  fn pretty_printing() -> anyhow::Result<()> {
    let p = PrettyPrinter::default();
    for event in iter_json_file(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/test.json")) {
      p.print(&event?);
    }
    Ok(())
  }
}
