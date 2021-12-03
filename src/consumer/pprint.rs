use std::fmt;
use ansi_term::Colour;
use crate::{Event, Level, FieldValue, EventKind};
use std::fmt::Formatter;

#[derive(Debug, Copy, Clone)]
pub struct PrettyPrinter {
  show_source: bool,
  target: bool,
}

#[derive(Debug, Copy, Clone)]
pub struct FmtEvent<'a> {
  printer: &'a PrettyPrinter,
  event: &'a Event,
}

impl Default for PrettyPrinter {
  fn default() -> Self {
    PrettyPrinter {
      show_source: true,
      target: true,
    }
  }
}

impl PrettyPrinter {
  pub fn show_source(mut self, on: bool) -> Self {
    self.show_source = on;
    self
  }
  pub fn target(mut self, on: bool) -> Self {
    self.target = on;
    self
  }
}


impl fmt::Display for FmtEvent<'_> {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    const CONT : &'static str = "  | ";
    let lvl = match self.event.level {
      Level::Trace => Colour::Purple.bold().paint("TRACE"),
      Level::Debug => Colour::Green.bold().paint("DEBUG"),
      Level::Info => Colour::Blue.bold().paint(" INFO"),
      Level::Warn => Colour::Yellow.bold().paint(" WARN"),
      Level::Error => Colour::Red.bold().paint("ERROR"),
    };

    f.write_fmt(format_args!("{}: ", lvl))?;

    match &self.event.kind {
      EventKind::Event(fields) => {
        if let Some(msg) = fields.get("message") {
          self.printer.fmt_fieldvalue(f, msg)?;
          if fields.len() > 1 {
            f.write_str("\n")?;
            f.write_str(CONT)?;
            self.printer.fmt_fields(f, fields.iter().filter(|(n, _)| n.as_str() != "message"))?;
          }
        } else {
          self.printer.fmt_fields(f, fields.iter())?;
        }
        f.write_str("\n")?;
      }
      _ => {
        // TODO print these (maybe move first up onto the LEVEL LINE?)
        //  Will need to print timings too, if they exist.
        f.write_str("<span event: TODO>\n")?;
      }
    }

    for span in self.event.spans.iter().rev() {
      f.write_fmt(format_args!(
        "{}{} {}{{",
        CONT,
        Colour::White.italic().paint("in"),
        Colour::White.bold().paint(&span.name),
      ))?;
      self.printer.fmt_fields(f, &span.fields)?;
      f.write_str("}\n")?;
    }


    if self.printer.target || self.printer.show_source {
      f.write_str(CONT)?;

      if self.printer.target {
        f.write_fmt(format_args!(
          "{} {} ",
          Colour::White.italic().paint("target"),
          Colour::White.bold().paint(&self.event.target)))?;
      }

      if self.printer.show_source {
        if let Some(file) = self.event.src_file.as_ref() {
          f.write_fmt(format_args!("{} {}", Colour::White.italic().paint("at"), file))?;
          if let Some(lineno) = self.event.src_line {
            f.write_fmt(format_args!(":L{}", lineno))?;
          }
        }
      }
      f.write_str("\n")?;
    }

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

  fn fmt_fieldvalue(&self, f: &mut Formatter, v: &FieldValue) -> fmt::Result {
    match v {
      FieldValue::Int(n) => f.write_fmt(format_args!("{}", Colour::Purple.paint(format!("{}", n))))?,
      FieldValue::Float(v) => f.write_fmt(format_args!("{}", Colour::Purple.paint(format!("{}", v))))?,
      FieldValue::Bool(v) => f.write_fmt(format_args!("{}", Colour::Yellow.paint(format!("{}", v))))?,
      FieldValue::Str(v) => f.write_fmt(format_args!("{}", v))?,
    };
    Ok(())
  }

  fn fmt_field(&self, f: &mut fmt::Formatter, field: (&str, &FieldValue)) -> fmt::Result {
    f.write_fmt(format_args!("{}= ", Colour::Blue.paint(field.0)))?;
    self.fmt_fieldvalue(f, field.1)
  }

  fn fmt_fields<'a, S, I>(&'a self, f: &mut Formatter, fields: I) -> fmt::Result
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
  use super::super::iter_logfile;



  #[test]
  fn pretty_printing() -> anyhow::Result<()> {
    let p = PrettyPrinter::default();
    for event in iter_logfile(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/test.json")) {
      p.print(&event?);
    }
    Ok(())
  }
}
