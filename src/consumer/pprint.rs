use crate::{Event, EventKind, FieldValue, Level, Span};
use ansi_term::Colour;
use std::fmt::{Display, Formatter, Result as FmtResult};
use std::num::NonZeroU64;

fn base64_id(id: NonZeroU64) -> [u8; 12] {
    const ALPHABET: &'static [u8] =
        "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/".as_bytes();

    // Mix the bits up (invertible hash function) to make the IDs look more different.
    // Shamelessly stolen from https://stackoverflow.com/questions/664014/what-integer-hash-function-are-good-that-accepts-an-integer-hash-key
    let mut id = u64::from(id);
    id = (id ^ (id >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    id = (id ^ (id >> 27)).wrapping_mul(0x94d049bb133111eb);
    id = id ^ (id >> 31);

    let mut output = [0; 12];
    let mut bytes = [0u8; 9];
    bytes[..8].copy_from_slice(&u64::from(id).to_be_bytes());

    let mut k = 0usize;

    for start in [0usize, 3, 6] {
        let chunk = [
            ((bytes[start] & 0b1111_1100) >> 2),
            ((bytes[start] & 0b0000_0011) << 4) | ((bytes[start + 1] & 0b1111_0000) >> 4),
            ((bytes[start + 1] & 0b0000_1111) << 2) | (bytes[start + 2] & 0b1100_0000 >> 6),
            (bytes[start + 2] & 0b0011_1111),
        ];

        for b in chunk {
            output[k] = ALPHABET[b as usize];
            k += 1;
        }
    }

    debug_assert_eq!(k, output.len());
    output
}

/// Configuration of pretty formatting for events.
#[derive(Debug, Copy, Clone)]
pub struct PrettyPrinter {
    source: bool,
    target: bool,
    span_times: bool,
    limit_spans: usize,
    span_ids: bool,
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
            span_ids: false,
            limit_spans: usize::MAX,
            continue_line: "  | ",
        }
    }
}

impl PrettyPrinter {
    /// Limit the number of spans per event printed.  The innermost spans will
    /// be display first.
    pub fn limit_spans(mut self, limit: usize) -> Self {
        self.limit_spans = limit;
        self
    }

    /// Show source file information
    pub fn show_source(mut self, on: bool) -> Self {
        self.source = on;
        self
    }

    /// Show span IDs as ID-strings
    pub fn show_span_ids(mut self, on: bool) -> Self {
        self.span_ids = on;
        self
    }

    /// Show target of the event
    pub fn show_target(mut self, on: bool) -> Self {
        self.target = on;
        self
    }

    /// Show span times for [`EventKind::SpanClose`] events.
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
                        self.printer.fmt_fields(
                            f,
                            fields.iter().filter(|(n, _)| n.as_str() != "message"),
                        )?;
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
                        write!(
                            f,
                            "{}: {:?} busy, {:?} idle\n",
                            verb,
                            times.busy(),
                            times.idle()
                        )?;
                    }
                    _ => {
                        write!(f, "{}\n", verb)?;
                    }
                }
            }
        }

        for span in spans {
            write!(
                f,
                "{}in {}\n",
                self.printer.continue_line,
                self.printer.fmt_span(span)
            )?;
        }

        if self.printer.target || self.printer.source {
            f.write_str(self.printer.continue_line)?;

            if self.printer.target {
                f.write_fmt(format_args!(
                    "{} {} ",
                    Colour::White.italic().paint("target"),
                    Colour::White.bold().paint(&self.event.target)
                ))?;
            }

            if self.printer.source {
                if let Some(file) = self.event.src_file.as_ref() {
                    f.write_fmt(format_args!(
                        "{} {}",
                        Colour::White.italic().paint("at"),
                        file
                    ))?;
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
        if self.printer.span_ids {
            if let Some(id) = self.span.id {
                let id = base64_id(id);
                write!(
                    f,
                    "{} ",
                    Colour::RGB(150, 150, 150).paint(std::str::from_utf8(&id).unwrap())
                )?;
            }
        }
        Colour::White.bold().paint(&self.span.name).fmt(f)?;
        f.write_str("{")?;
        self.printer.fmt_fields(f, &self.span.fields)?;
        f.write_str("}")?;
        Ok(())
    }
}

impl PrettyPrinter {
    /// Format an event for pretty-printing
    pub fn fmt<'a>(&'a self, event: &'a Event) -> FmtEvent<'a> {
        FmtEvent {
            printer: self,
            event,
        }
    }

    /// Convenience method for `println!("{}", printer.fmt(event))`
    pub fn print(&self, event: &Event) {
        println!("{}", self.fmt(event));
    }

    fn fmt_span<'a>(&'a self, span: &'a Span) -> FmtSpan<'a> {
        FmtSpan {
            printer: self,
            span,
        }
    }

    fn fmt_fieldvalue(&self, f: &mut Formatter, v: &FieldValue) -> FmtResult {
        match v {
            FieldValue::Int(n) => {
                f.write_fmt(format_args!("{}", Colour::Purple.paint(format!("{}", n))))?
            }
            FieldValue::Float(v) => {
                f.write_fmt(format_args!("{}", Colour::Purple.paint(format!("{}", v))))?
            }
            FieldValue::Bool(v) => {
                f.write_fmt(format_args!("{}", Colour::Yellow.paint(format!("{}", v))))?
            }
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
        I: IntoIterator<Item = (&'a S, &'a FieldValue)> + 'a,
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

#[cfg(all(test, feature = "consumer"))]
mod tests {
    use super::*;
    use crate::consumer::*;
    use crate::format::Json;

    #[test]
    fn pretty_printing() -> anyhow::Result<()> {
        let p = PrettyPrinter::default();
        for event in Json.iter_file(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/test.json")) {
            p.print(&event?);
        }
        Ok(())
    }
}
